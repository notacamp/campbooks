import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  Button,
  DataGrid,
  IconButton,
  MenuItem,
  MenuList,
  Popover,
  SummaryButton,
  type CellPos,
  type GridColumn,
} from "~/lib/ui";

import type { PageSummary, UploadedFile } from "../../types";
import { CellDisplay } from "../cells/CellDisplay";
import { CellEditor } from "../cells/CellEditor";
import type { CellContext } from "../cells/context";
import { useSyncedTable } from "../notion/context";
import { CellConflictFlag } from "../notion/CellConflict";
import { cellKey, campbooksOnly } from "../notion/model";
import { RemoveColumnDialog } from "../notion/RemoveColumnDialog";
import {
  columnsOf,
  configJson,
  editable as typeEditable,
  formatSummary,
  specsOf,
  summaryKinds,
  summaryKey,
  viewConfig,
  type Column,
} from "../model";
import type { TableActions } from "../use-table-actions";
import type {
  PropertyType,
  PropertyView,
  RowView,
  SortDirection,
  SummaryKind,
  SummaryValue,
  TableViewView,
} from "../types";

import { ColumnMenu, NewColumn, type TableChoice } from "./ColumnMenus";

export interface GridPaneProps {
  ctx: CellContext;
  view: TableViewView;
  properties: readonly PropertyView[];
  rows: readonly RowView[];
  total: number;
  summaries: readonly SummaryValue[];
  pages: readonly PageSummary[];
  tables: readonly TableChoice[];
  here: TableChoice;
  actions: TableActions;
  canWrite: boolean;
  editing: { position: CellPos; seed?: string | undefined } | null;
  onEditing: (editing: { position: CellPos; seed?: string | undefined } | null) => void;
  reveal: CellPos | undefined;
  onActiveRow: (row: string | undefined) => void;
  onOpenRow: (row: string) => void;
  onFilter: (property: string) => void;
  hasMore: boolean;
  loadingMore: boolean;
  onLoadMore: () => void;
  upload: (row: string, file: File) => Promise<UploadedFile>;
  onNewColumn: (
    name: string,
    type: PropertyType,
    extra: { table?: string; pairedName?: string },
  ) => void;
  onNewRow: () => void;
  /** How many rows the table has (a removed column takes a value from each). */
  tableRows: number;
}

/** The grid of a table view: header, cells that edit in place, summaries and the new-row cell. */
export function GridPane({
  ctx,
  view,
  properties,
  rows,
  total,
  summaries,
  pages,
  tables,
  here,
  actions,
  canWrite,
  editing,
  onEditing,
  reveal,
  onActiveRow,
  onOpenRow,
  onFilter,
  hasMore,
  loadingMore,
  onLoadMore,
  upload,
  onNewColumn,
  onNewRow,
  tableRows,
}: GridPaneProps) {
  const { t } = useTranslation("pages");
  const synced = useSyncedTable();
  const [removing, setRemoving] = useState<PropertyView | undefined>();
  const config = viewConfig(view);
  const columns = columnsOf(properties, config);
  const shown = columns.filter((column) => column.visible);
  const rowById = useMemo(() => new Map(rows.map((row) => [row.id, row])), [rows]);

  const save = (next: Partial<typeof config>) => {
    void actions.updateView(view, {
      config: configJson({ ...config, ...next }, view.type === "board"),
    });
  };
  // Every column named in the view, in its order: the shape the server keeps.
  const withColumns = (change: (column: Column) => Column | undefined) =>
    specsOf(columns.flatMap((column) => change(column) ?? []));

  const sortOf = (property: string): SortDirection | undefined =>
    config.sorts.find((sort) => sort.property === property)?.direction;

  // A column of a table kept in step with Notion is removed there too, and only when the member
  // says so. A status column may be only Campbooks': it is tried plain first, and the server's
  // "confirmation_required" is what opens the dialog.
  const removeColumn = (property: PropertyView) => {
    if (!synced.syncing || campbooksOnly(property.type)) {
      void actions.removeProperty(property);
    } else if (property.type === "status") {
      void actions.removeProperty(property, { probe: true }).then((result) => {
        if (result === "confirm") {
          setRemoving(property);
        }
      });
    } else {
      setRemoving(property);
    }
  };

  const gridColumns: GridColumn[] = shown.map((column) => ({
    id: column.property.id,
    width: column.width,
    label: column.property.name,
    header: (
      <ColumnMenu
        property={column.property}
        sort={sortOf(column.property.id)}
        canWrite={canWrite}
        onRename={(name) => {
          void actions.renameProperty(column.property, name);
        }}
        onType={(type) => {
          void actions.changeType(column.property, type);
        }}
        onConfig={(next) => {
          void actions.updateConfig(column.property, next);
        }}
        onSort={(direction) => {
          const others = config.sorts.filter((sort) => sort.property !== column.property.id);
          save({
            sorts:
              direction === undefined
                ? others
                : [{ property: column.property.id, direction }, ...others].slice(0, 5),
          });
        }}
        onFilter={() => {
          onFilter(column.property.id);
        }}
        onHide={() => {
          save({
            properties: withColumns((candidate) =>
              candidate.property.id === column.property.id
                ? { ...candidate, visible: false }
                : candidate,
            ),
          });
        }}
        onDelete={() => {
          removeColumn(column.property);
        }}
      />
    ),
  }));

  const cellEditable = (property: PropertyView) =>
    property.type === "title" || typeEditable(property.type);

  const toggleCheckbox = (row: RowView, property: PropertyView) => {
    void actions.setCell(row, property, row.cells[property.id]?.value !== true);
  };

  const renderCell = (
    gridRow: { id: string },
    gridColumn: GridColumn,
    state: { editing: boolean },
  ) => {
    const row = rowById.get(gridRow.id);
    const property = shown.find((column) => column.property.id === gridColumn.id)?.property;
    if (row === undefined || property === undefined) {
      return null;
    }
    const value = property.type === "title" ? row.title : row.cells[property.id]?.value;
    const conflict = synced.cells.get(cellKey(row.id, property.id));
    if (conflict !== undefined) {
      return (
        <>
          <CellDisplay ctx={ctx} property={property} row={row} value={value} />
          <span className="ml-auto flex-none">
            <CellConflictFlag
              ctx={ctx}
              conflict={conflict}
              property={property}
              row={row}
              onChoose={
                canWrite && synced.canResolve
                  ? (keep) => synced.resolve(conflict.id, keep)
                  : undefined
              }
            />
          </span>
        </>
      );
    }
    if (state.editing && canWrite && cellEditable(property)) {
      return (
        <CellEditor
          ctx={ctx}
          property={property}
          row={row}
          value={value}
          seed={editing?.seed}
          pages={pages}
          canEditOptions={canWrite}
          onCancel={() => {
            onEditing(null);
          }}
          onCommit={(next) => {
            onEditing(null);
            if (property.type === "title") {
              void actions.setTitle(row, typeof next === "string" ? next : row.title);
            } else {
              void actions.setCell(row, property, next);
            }
          }}
          onOptions={(next, previous) => actions.setOptions(property, next, previous)}
          upload={upload}
        />
      );
    }
    if (property.type === "checkbox") {
      return (
        <input
          type="checkbox"
          aria-label={`${property.name} — ${row.title === "" ? t("untitled") : row.title}`}
          checked={value === true}
          disabled={!canWrite}
          onChange={() => {
            toggleCheckbox(row, property);
          }}
        />
      );
    }
    if (property.type === "title") {
      return (
        <>
          <CellDisplay ctx={ctx} property={property} row={row} value={value} />
          <span className="ml-auto flex-none">
            <IconButton
              icon="preview"
              label={t("tables.row.open", { title: row.title === "" ? t("untitled") : row.title })}
              onClick={() => {
                onOpenRow(row.id);
              }}
            />
          </span>
        </>
      );
    }
    return <CellDisplay ctx={ctx} property={property} row={row} value={value} />;
  };

  const summaryOf = (property: PropertyView) => {
    const kind = config.summaries.find((summary) => summary.property === property.id)?.kind;
    if (kind === undefined) {
      return { kind: undefined, text: "" };
    }
    const found = summaries.find(
      (summary) => summary.property === property.id && summary.kind === kind,
    );
    return { kind, text: formatSummary(kind, found?.value, property, ctx.locale) };
  };

  const setSummary = (property: PropertyView, kind: SummaryKind | undefined) => {
    const others = config.summaries.filter((summary) => summary.property !== property.id);
    save({ summaries: kind === undefined ? others : [...others, { property: property.id, kind }] });
  };

  return (
    <>
      <DataGrid
        label={here.title === "" ? t("untitled") : here.title}
        columns={gridColumns}
        rows={rows.map((row) => ({
          id: row.id,
          label: row.title === "" ? t("untitled") : row.title,
        }))}
        renderCell={renderCell}
        editing={editing?.position ?? null}
        reveal={reveal}
        onEdit={(position, seed) => {
          const row = rowById.get(position.rowId);
          const property = shown.find(
            (column) => column.property.id === position.columnId,
          )?.property;
          if (row === undefined || property === undefined || !canWrite || !cellEditable(property)) {
            return;
          }
          if (synced.cells.has(cellKey(row.id, property.id))) {
            return;
          }
          if (property.type === "checkbox") {
            if (seed === undefined || seed === " ") {
              toggleCheckbox(row, property);
            }
            return;
          }
          onEditing({ position, seed });
        }}
        onClear={(position) => {
          const row = rowById.get(position.rowId);
          const property = shown.find(
            (column) => column.property.id === position.columnId,
          )?.property;
          if (
            row !== undefined &&
            property !== undefined &&
            canWrite &&
            property.type !== "title" &&
            typeEditable(property.type) &&
            !synced.cells.has(cellKey(row.id, property.id)) &&
            row.cells[property.id] !== undefined
          ) {
            void actions.setCell(row, property, null);
          }
        }}
        onActiveChange={(position) => {
          onActiveRow(position?.rowId);
        }}
        empty={t("tables.rows.empty")}
        onResize={
          canWrite
            ? (id, width) => {
                save({
                  properties: withColumns((candidate) =>
                    candidate.property.id === id ? { ...candidate, width } : candidate,
                  ),
                });
              }
            : undefined
        }
        resizeLabel={(name) => t("tables.column.resize", { property: name })}
        headerEndLabel={t("tables.column.add")}
        headerEnd={
          canWrite ? <NewColumn tables={tables} here={here} onCreate={onNewColumn} /> : undefined
        }
        footerStart={
          canWrite ? (
            <Button
              variant="ghost"
              icon="plus"
              shortcut="pages.table.new_row"
              onClick={() => {
                onNewRow();
              }}
            >
              {t("tables.rows.new")}
            </Button>
          ) : (
            <span>{t("tables.rows.count", { count: total })}</span>
          )
        }
        renderFooter={(column) => {
          const property = shown.find((candidate) => candidate.property.id === column.id)?.property;
          if (property === undefined) {
            return null;
          }
          const current = summaryOf(property);
          return (
            <Popover
              label={t("tables.summary.menu", { property: property.name })}
              placement="top-start"
              layer="top"
              renderTrigger={(trigger) => (
                <SummaryButton
                  {...trigger}
                  label={t("tables.summary.menu", { property: property.name })}
                  text={
                    current.kind === undefined
                      ? undefined
                      : `${t(`tables.summary.${summaryKey(current.kind)}`)} ${current.text}`
                  }
                />
              )}
            >
              {(close) => (
                <MenuList label={t("tables.summary.menu", { property: property.name })}>
                  <MenuItem
                    checked={current.kind === undefined}
                    onSelect={() => {
                      close();
                      setSummary(property, undefined);
                    }}
                  >
                    {t("tables.summary.none")}
                  </MenuItem>
                  {summaryKinds(property.type).map((kind) => (
                    <MenuItem
                      key={kind}
                      checked={current.kind === kind}
                      onSelect={() => {
                        close();
                        setSummary(property, kind);
                      }}
                    >
                      {t(`tables.summary.${summaryKey(kind)}`)}
                    </MenuItem>
                  ))}
                </MenuList>
              )}
            </Popover>
          );
        }}
        hasMore={hasMore}
        loadingMore={loadingMore}
        onLoadMore={onLoadMore}
        loadMoreLabel={t("tables.rows.loadMore", { shown: rows.length, total })}
      />
      <RemoveColumnDialog
        name={removing?.name}
        rows={tableRows}
        onClose={() => {
          setRemoving(undefined);
        }}
        onConfirm={async () => {
          if (removing === undefined) {
            return false;
          }
          const result = await actions.removeProperty(removing, { confirm: true });
          return result === "removed";
        }}
      />
    </>
  );
}
