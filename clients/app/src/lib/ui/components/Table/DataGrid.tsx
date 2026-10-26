import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type MouseEvent,
  type ReactNode,
} from "react";

import { Button } from "@notacamp/ui";

import styles from "./DataGrid.module.css";

export interface GridColumn {
  id: string;
  /** Its width in px. */
  width: number;
  /** What the header shows (a button that opens the column's menu). */
  header: ReactNode;
  /** The column's name for screen readers. */
  label: string;
}

export interface GridRow {
  id: string;
  /** The row's name for screen readers (its title). */
  label: string;
}

export interface CellPos {
  rowId: string;
  columnId: string;
}

export interface CellState {
  active: boolean;
  editing: boolean;
}

export interface DataGridProps {
  /** Names the grid ("Clients"). */
  label: string;
  columns: readonly GridColumn[];
  rows: readonly GridRow[];
  renderCell: (row: GridRow, column: GridColumn, state: CellState) => ReactNode;
  /** The cell being edited, if any. */
  editing: CellPos | null;
  /** Enter, F2, a click on the active cell or a typed character starts editing; `seed` is the typed character. */
  onEdit: (position: CellPos, seed?: string) => void;
  /** Delete or Backspace on a cell. */
  onClear: (position: CellPos) => void;
  /** The active cell moved (arrows, a click). */
  onActiveChange?: ((position: CellPos | null) => void) | undefined;
  /** The footer's cell for a column (its summary). */
  renderFooter?: ((column: GridColumn) => ReactNode) | undefined;
  /** The cell at the end of the header (adds a column), and its name. */
  headerEnd?: ReactNode | undefined;
  headerEndLabel?: string | undefined;
  /** The footer's first cell (adds a row). */
  footerStart?: ReactNode | undefined;
  /** More rows can be loaded. */
  hasMore?: boolean | undefined;
  loadingMore?: boolean | undefined;
  onLoadMore?: (() => void) | undefined;
  /** The load-more button's text, and the screen-reader summary of the grid's size. */
  loadMoreLabel?: string | undefined;
  /** Said under the header when there are no rows. */
  empty?: ReactNode | undefined;
  /** Rows past this many are windowed (only the ones in view are drawn). */
  windowAfter?: number | undefined;
  /** Move the active cell here and focus it (a row that was just made). */
  reveal?: CellPos | undefined;
  /** A column was resized (by dragging its edge, or with the arrow keys on it). */
  onResize?: ((columnId: string, width: number) => void) | undefined;
  /** The resize handle's name for a column ("Resize Stage"). */
  resizeLabel?: ((column: string) => string) | undefined;
}

const MIN_COLUMN = 64;
const MAX_COLUMN = 640;
const RESIZE_STEP = 16;

const OVERSCAN = 6;
const PAGE_ROWS = 10;

/** The cell at a row and column, wherever the grid has drawn it. */
function cellElement(grid: HTMLElement | null, row: number, column: number): HTMLElement | null {
  return (
    [...(grid?.querySelectorAll<HTMLElement>('[role="gridcell"][data-row]') ?? [])].find(
      (cell) => cell.dataset["row"] === String(row) && cell.dataset["column"] === String(column),
    ) ?? null
  );
}

/**
 * A grid of rows and columns: header and first column stay in view, the box
 * scrolls sideways (and down, past a height) so the page never does, and one
 * cell is the tab stop (roving focus): arrows, Home, End and Page Up/Down
 * move, Enter or F2 or typing edits, Delete clears. Past `windowAfter` rows
 * only the rows in view are drawn.
 */
export function DataGrid({
  label,
  columns,
  rows,
  renderCell,
  editing,
  onEdit,
  onClear,
  onActiveChange,
  renderFooter,
  headerEnd,
  headerEndLabel,
  footerStart,
  hasMore = false,
  loadingMore = false,
  onLoadMore,
  loadMoreLabel,
  empty,
  windowAfter = 120,
  reveal,
  onResize,
  resizeLabel,
}: DataGridProps) {
  const scroller = useRef<HTMLDivElement>(null);
  const [active, setActive] = useState({ row: 0, column: 0 });
  // The first body row's height, once laid out: windowing needs it.
  const [rowHeight, setRowHeight] = useState(0);
  const [scroll, setScroll] = useState({ top: 0, height: 0 });

  const [resizing, setResizing] = useState<{ id: string; width: number } | undefined>();
  const rowCount = rows.length;
  const columnCount = columns.length;
  const at = {
    row: Math.min(active.row, Math.max(0, rowCount - 1)),
    column: Math.min(active.column, Math.max(0, columnCount - 1)),
  };

  const windowed = rowCount > windowAfter && rowHeight > 0;
  const first = windowed ? Math.max(0, Math.floor(scroll.top / rowHeight) - OVERSCAN) : 0;
  const last = windowed
    ? Math.min(rowCount, Math.ceil((scroll.top + scroll.height) / rowHeight) + OVERSCAN)
    : rowCount;

  useLayoutEffect(() => {
    const box = scroller.current;
    if (box === null) {
      return;
    }
    const sample = box.querySelector<HTMLElement>('[data-body-row="true"]');
    if (sample !== null && sample.offsetHeight > 0) {
      setRowHeight(sample.offsetHeight);
    }
    setScroll((current) =>
      current.height === box.clientHeight ? current : { ...current, height: box.clientHeight },
    );
  }, [rowCount, columnCount]);

  const position = useCallback(
    (row: number, column: number): CellPos | null => {
      const r = rows[row];
      const c = columns[column];
      return r === undefined || c === undefined ? null : { rowId: r.id, columnId: c.id };
    },
    [rows, columns],
  );

  const move = (row: number, column: number, focus = true) => {
    const next = {
      row: Math.min(Math.max(0, row), Math.max(0, rowCount - 1)),
      column: Math.min(Math.max(0, column), Math.max(0, columnCount - 1)),
    };
    setActive(next);
    onActiveChange?.(position(next.row, next.column));
    const box = scroller.current;
    if (windowed && box !== null && rowHeight > 0) {
      const top = next.row * rowHeight;
      if (top < box.scrollTop) {
        box.scrollTop = top;
      } else if (top + rowHeight > box.scrollTop + box.clientHeight) {
        box.scrollTop = top + rowHeight - box.clientHeight;
      }
    }
    if (focus) {
      // After the row is drawn (windowing may not have it yet).
      requestAnimationFrame(() => {
        cellElement(scroller.current, next.row, next.column)?.focus({ preventScroll: false });
      });
    }
  };

  // After editing ends, the cell gets the focus back.
  const wasEditing = useRef(false);
  useEffect(() => {
    if (editing === null && wasEditing.current) {
      cellElement(scroller.current, at.row, at.column)?.focus();
    }
    wasEditing.current = editing !== null;
  }, [editing, at.row, at.column]);

  // A row that was just made: the active cell goes there (state adjusted while rendering).
  const [revealed, setRevealed] = useState<CellPos | undefined>();
  if (reveal !== undefined && reveal !== revealed) {
    const row = rows.findIndex((candidate) => candidate.id === reveal.rowId);
    const column = columns.findIndex((candidate) => candidate.id === reveal.columnId);
    if (row !== -1 && column !== -1) {
      setRevealed(reveal);
      setActive({ row, column });
    }
  }
  const announced = useRef<CellPos | undefined>(undefined);
  const latestActive = useRef(onActiveChange);
  useEffect(() => {
    latestActive.current = onActiveChange;
  });
  useEffect(() => {
    if (revealed === undefined || announced.current === revealed) {
      return;
    }
    announced.current = revealed;
    latestActive.current?.(revealed);
    const row = rows.findIndex((candidate) => candidate.id === revealed.rowId);
    const column = columns.findIndex((candidate) => candidate.id === revealed.columnId);
    // After the row is drawn; an editor already open in it keeps the focus.
    requestAnimationFrame(() => {
      const cell = cellElement(scroller.current, row, column);
      if (cell !== null && !cell.contains(document.activeElement)) {
        cell.focus();
      }
    });
  }, [revealed, rows, columns]);

  // Loading more as the end comes into view.
  const nearEnd =
    rowHeight > 0 && scroll.top + scroll.height > rowCount * rowHeight - 5 * rowHeight;
  useEffect(() => {
    if (hasMore && !loadingMore && onLoadMore !== undefined && windowed && nearEnd) {
      onLoadMore();
    }
  }, [hasMore, loadingMore, onLoadMore, windowed, nearEnd]);

  const onCellKeyDown = (event: KeyboardEvent<HTMLDivElement>, row: number, column: number) => {
    // Only the cell itself: an editor inside it keeps its own keys.
    if (event.target !== event.currentTarget) {
      return;
    }
    const current = position(row, column);
    const ctrl = event.ctrlKey || event.metaKey;
    switch (event.key) {
      case "ArrowDown": {
        event.preventDefault();
        move(row + 1, column);
        return;
      }
      case "ArrowUp": {
        event.preventDefault();
        move(row - 1, column);
        return;
      }
      case "ArrowRight": {
        event.preventDefault();
        move(row, column + 1);
        return;
      }
      case "ArrowLeft": {
        event.preventDefault();
        move(row, column - 1);
        return;
      }
      case "Home": {
        event.preventDefault();
        move(ctrl ? 0 : row, 0);
        return;
      }
      case "End": {
        event.preventDefault();
        move(ctrl ? rowCount - 1 : row, columnCount - 1);
        return;
      }
      case "PageDown": {
        event.preventDefault();
        move(row + PAGE_ROWS, column);
        return;
      }
      case "PageUp": {
        event.preventDefault();
        move(row - PAGE_ROWS, column);
        return;
      }
      case "Enter":
      case "F2": {
        event.preventDefault();
        if (current !== null) {
          onEdit(current);
        }
        return;
      }
      case "Delete":
      case "Backspace": {
        event.preventDefault();
        if (current !== null) {
          onClear(current);
        }
        return;
      }
      default: {
        if (current !== null && event.key.length === 1 && !ctrl && !event.altKey) {
          event.preventDefault();
          onEdit(current, event.key);
        }
      }
    }
  };

  const onClickCell = (event: MouseEvent<HTMLElement>, row: number, column: number) => {
    // A control inside the cell (a checkbox, a chip, a button) does its own job.
    const inner = (event.target as HTMLElement).closest("a, button, input, label");
    const wasActive = at.row === row && at.column === column;
    move(row, column, inner === null);
    const target = position(row, column);
    if (target !== null && wasActive && inner === null) {
      onEdit(target);
    }
  };

  const widthOf = (column: GridColumn) =>
    resizing?.id === column.id ? resizing.width : column.width;
  const total = columns.reduce((sum, column) => sum + widthOf(column), 0);
  const style = {
    "--grid-columns": columns.map((column) => `${String(widthOf(column))}px`).join(" "),
    "--grid-width": `${String(total)}px`,
  } as CSSProperties;

  const visible = rows.slice(first, last);
  const before = first * rowHeight;
  const after = windowed ? (rowCount - last) * rowHeight : 0;

  return (
    <div className={styles["frame"]}>
      <div
        ref={scroller}
        className={styles["scroller"]}
        style={style}
        onScroll={(event) => {
          const box = event.currentTarget;
          if (windowed || hasMore) {
            setScroll({ top: box.scrollTop, height: box.clientHeight });
          }
        }}
      >
        <div
          role="grid"
          aria-label={label}
          aria-rowcount={rowCount + 1}
          aria-colcount={columnCount + (headerEnd === undefined ? 0 : 1)}
          className={styles["grid"]}
        >
          <div role="row" aria-rowindex={1} className={styles["row"]} data-head="true">
            {columns.map((column, index) => (
              <div
                key={column.id}
                role="columnheader"
                aria-colindex={index + 1}
                aria-label={column.label}
                className={styles["headCell"]}
                data-first={index === 0 || undefined}
              >
                {column.header}
                {onResize === undefined ? null : (
                  <ResizeHandle
                    label={resizeLabel?.(column.label) ?? column.label}
                    width={widthOf(column)}
                    onChange={(width) => {
                      setResizing({ id: column.id, width });
                    }}
                    onCommit={(width) => {
                      setResizing(undefined);
                      onResize(column.id, width);
                    }}
                  />
                )}
              </div>
            ))}
            {headerEnd === undefined ? null : (
              <div
                role="columnheader"
                aria-colindex={columnCount + 1}
                aria-label={headerEndLabel}
                className={styles["headEnd"]}
              >
                {headerEnd}
              </div>
            )}
          </div>
          {before > 0 ? <div aria-hidden="true" style={{ height: before }} /> : null}
          {visible.map((row, offset) => {
            const rowIndex = first + offset;
            return (
              <div
                key={row.id}
                role="row"
                aria-rowindex={rowIndex + 2}
                aria-label={row.label}
                className={styles["row"]}
                data-body-row="true"
                data-active={at.row === rowIndex || undefined}
              >
                {columns.map((column, columnIndex) => {
                  const isActive = at.row === rowIndex && at.column === columnIndex;
                  const isEditing = editing?.rowId === row.id && editing.columnId === column.id;
                  return (
                    <div
                      key={column.id}
                      role="gridcell"
                      aria-colindex={columnIndex + 1}
                      tabIndex={isActive ? 0 : -1}
                      className={styles["cell"]}
                      data-row={rowIndex}
                      data-column={columnIndex}
                      data-first={columnIndex === 0 || undefined}
                      data-active={isActive || undefined}
                      data-editing={isEditing || undefined}
                      onKeyDown={(event) => {
                        onCellKeyDown(event, rowIndex, columnIndex);
                      }}
                      onClick={(event) => {
                        onClickCell(event, rowIndex, columnIndex);
                      }}
                      onDoubleClick={() => {
                        const target = position(rowIndex, columnIndex);
                        if (target !== null && !isEditing) {
                          onEdit(target);
                        }
                      }}
                      onFocus={(event) => {
                        // Tabbing in lands on the active cell; a click or a script focus
                        // on another one makes that the active cell.
                        if (event.target === event.currentTarget && !isActive) {
                          setActive({ row: rowIndex, column: columnIndex });
                          onActiveChange?.(position(rowIndex, columnIndex));
                        }
                      }}
                    >
                      {renderCell(row, column, { active: isActive, editing: isEditing })}
                    </div>
                  );
                })}
              </div>
            );
          })}
          {after > 0 ? <div aria-hidden="true" style={{ height: after }} /> : null}
          {rowCount === 0 && empty !== undefined ? (
            <div className={styles["empty"]}>{empty}</div>
          ) : null}
          {renderFooter === undefined && footerStart === undefined ? null : (
            <div role="row" aria-rowindex={rowCount + 2} className={styles["row"]} data-foot="true">
              {columns.map((column, index) => (
                <div
                  key={column.id}
                  role="gridcell"
                  aria-colindex={index + 1}
                  className={styles["footCell"]}
                  data-first={index === 0 || undefined}
                >
                  {index === 0 ? footerStart : renderFooter?.(column)}
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
      {hasMore && loadMoreLabel !== undefined ? (
        <div className={styles["more"]}>
          <Button variant="ghost" disabled={loadingMore} onClick={onLoadMore}>
            {loadMoreLabel}
          </Button>
        </div>
      ) : null}
    </div>
  );
}

interface ResizeHandleProps {
  label: string;
  width: number;
  onChange: (width: number) => void;
  onCommit: (width: number) => void;
}

/** The edge of a header cell: drag it, or focus it and press ← and →. */
function ResizeHandle({ label, width, onChange, onCommit }: ResizeHandleProps) {
  const clamp = (value: number) => Math.min(MAX_COLUMN, Math.max(MIN_COLUMN, Math.round(value)));
  const start = useRef<{ x: number; width: number } | undefined>(undefined);
  return (
    <div
      role="slider"
      tabIndex={0}
      aria-orientation="horizontal"
      aria-label={label}
      aria-valuemin={MIN_COLUMN}
      aria-valuemax={MAX_COLUMN}
      aria-valuenow={width}
      className={styles["resize"]}
      onPointerDown={(event) => {
        event.currentTarget.setPointerCapture(event.pointerId);
        start.current = { x: event.clientX, width };
      }}
      onPointerMove={(event) => {
        if (start.current !== undefined) {
          onChange(clamp(start.current.width + event.clientX - start.current.x));
        }
      }}
      onPointerUp={(event) => {
        if (start.current !== undefined) {
          onCommit(clamp(start.current.width + event.clientX - start.current.x));
          start.current = undefined;
        }
      }}
      onKeyDown={(event) => {
        if (event.key === "ArrowRight" || event.key === "ArrowLeft") {
          event.preventDefault();
          event.stopPropagation();
          onCommit(clamp(width + (event.key === "ArrowRight" ? RESIZE_STEP : -RESIZE_STEP)));
        }
      }}
    />
  );
}
