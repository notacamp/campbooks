import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import { Board, type BoardGroup } from "./Board";
import {
  CellButton,
  CellEmpty,
  CellInput,
  CellLine,
  CellLink,
  HeaderButton,
  OptionTag,
  SummaryButton,
  TypeGlyph,
} from "./Cells";
import { DataGrid, type CellPos, type GridColumn } from "./DataGrid";
import { CheckGroup, MenuHeading, MenuItem, MenuList, MenuSeparator } from "./Menu";
import { Picker } from "./Picker";
import { PropertyLine, PropertyList, RowPeek } from "./Peek";
import { FilterChip, TableToolbar, ToolbarButton } from "./Toolbar";
import { must } from "../../../../../test/must";

describe("the menu", () => {
  it("moves between items with the arrows, Home and End", () => {
    const onSelect = vi.fn();
    renderUi(
      <MenuList label="Column">
        <MenuHeading>Sort</MenuHeading>
        <MenuItem onSelect={onSelect} glyph="Aa" hint="plain">
          First
        </MenuItem>
        <MenuSeparator />
        <MenuItem onSelect={onSelect} checked>
          Second
        </MenuItem>
        <MenuItem onSelect={onSelect} disabled>
          Third
        </MenuItem>
        <MenuItem onSelect={onSelect} danger icon="trash">
          Fourth
        </MenuItem>
      </MenuList>,
    );
    const menu = screen.getByRole("menu", { name: "Column" });
    const first = screen.getByRole("menuitem", { name: /^First/u });
    first.focus();
    fireEvent.keyDown(menu, { key: "ArrowDown" });
    expect(screen.getByRole("menuitemradio", { name: "Second" })).toHaveFocus();
    // The disabled one is skipped.
    fireEvent.keyDown(menu, { key: "ArrowDown" });
    expect(screen.getByRole("menuitem", { name: "Fourth" })).toHaveFocus();
    fireEvent.keyDown(menu, { key: "ArrowDown" });
    expect(first).toHaveFocus();
    fireEvent.keyDown(menu, { key: "ArrowUp" });
    expect(screen.getByRole("menuitem", { name: "Fourth" })).toHaveFocus();
    fireEvent.keyDown(menu, { key: "Home" });
    expect(first).toHaveFocus();
    fireEvent.keyDown(menu, { key: "End" });
    expect(screen.getByRole("menuitem", { name: "Fourth" })).toHaveFocus();
    fireEvent.keyDown(menu, { key: "x" });
    fireEvent.click(first);
    expect(onSelect).toHaveBeenCalledTimes(1);
  });

  it("does nothing for keys in an empty menu, and groups checkboxes under a legend", () => {
    renderUi(
      <>
        <MenuList label="Empty">{null}</MenuList>
        <CheckGroup legend="Columns">
          <input type="checkbox" aria-label="One" />
        </CheckGroup>
      </>,
    );
    fireEvent.keyDown(screen.getByRole("menu", { name: "Empty" }), { key: "ArrowDown" });
    expect(screen.getByRole("group", { name: "Columns" })).toBeTruthy();
  });
});

describe("the picker", () => {
  const items = [
    { id: "a", text: "Alpha", label: <span>Alpha</span>, selected: true, hint: "first" },
    {
      id: "b",
      text: "Beta",
      label: <span>Beta</span>,
      end: <button type="button">Edit Beta</button>,
    },
  ];

  it("picks with the mouse and with Enter on the highlighted item", () => {
    const onPick = vi.fn();
    renderUi(
      <Picker
        label="Letters"
        searchLabel="Search letters"
        query=""
        onQuery={vi.fn()}
        items={items}
        onPick={onPick}
        empty="None"
        multiple
      />,
    );
    const search = screen.getByRole("combobox", { name: "Search letters" });
    expect(screen.getByRole("listbox", { name: "Letters" })).toHaveAttribute(
      "aria-multiselectable",
      "true",
    );
    expect(screen.getByRole("option", { name: "Alpha", selected: true })).toBeTruthy();
    fireEvent.mouseDown(screen.getByRole("option", { name: "Beta" }));
    fireEvent.click(screen.getByRole("option", { name: "Beta" }));
    expect(onPick).toHaveBeenLastCalledWith("b");
    fireEvent.keyDown(search, { key: "ArrowDown" });
    fireEvent.keyDown(search, { key: "Enter" });
    expect(onPick).toHaveBeenLastCalledWith("b");
    fireEvent.keyDown(search, { key: "ArrowUp" });
    fireEvent.keyDown(search, { key: "Enter" });
    expect(onPick).toHaveBeenLastCalledWith("a");
    fireEvent.keyDown(screen.getByRole("option", { name: "Alpha" }), { key: " " });
    expect(onPick).toHaveBeenLastCalledWith("a");
    fireEvent.keyDown(screen.getByRole("option", { name: "Alpha" }), { key: "Tab" });
  });

  it("offers to create what was typed, says when there is nothing, and when it is loading", () => {
    const onCreate = vi.fn();
    const { rerender } = renderUi(
      <Picker
        label="Letters"
        searchLabel="Search letters"
        query="gam"
        onQuery={vi.fn()}
        items={[]}
        onPick={vi.fn()}
        empty="None"
        create={{ label: "Create gam", onCreate }}
        footer={<button type="button">Clear</button>}
      />,
    );
    fireEvent.click(screen.getByRole("option", { name: "Create gam" }));
    expect(onCreate).toHaveBeenCalledTimes(1);
    fireEvent.keyDown(screen.getByRole("option", { name: "Create gam" }), { key: "Enter" });
    expect(onCreate).toHaveBeenCalledTimes(2);
    fireEvent.keyDown(screen.getByRole("combobox"), { key: "Enter" });
    expect(onCreate).toHaveBeenCalledTimes(3);
    expect(screen.getByRole("button", { name: "Clear" })).toBeTruthy();
    rerender(
      <Picker
        label="Letters"
        searchLabel="Search"
        query=""
        onQuery={vi.fn()}
        items={[]}
        onPick={vi.fn()}
        empty="None"
      />,
    );
    expect(screen.getByRole("status")).toHaveTextContent("None");
    rerender(
      <Picker
        label="Letters"
        searchLabel="Search"
        query=""
        onQuery={vi.fn()}
        items={[]}
        onPick={vi.fn()}
        empty="None"
        loading="Searching…"
      />,
    );
    expect(screen.getByRole("status")).toHaveTextContent("Searching…");
    fireEvent.keyDown(screen.getByRole("combobox"), { key: "ArrowDown" });
    fireEvent.keyDown(screen.getByRole("combobox"), { key: "ArrowUp" });
    fireEvent.keyDown(screen.getByRole("combobox"), { key: "Enter" });
  });

  it("reports what is typed", () => {
    const onQuery = vi.fn();
    renderUi(
      <Picker
        label="L"
        searchLabel="Search"
        query=""
        onQuery={onQuery}
        items={items}
        onPick={vi.fn()}
        empty="None"
      />,
    );
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "be" } });
    expect(onQuery).toHaveBeenCalledWith("be");
  });
});

describe("the small parts of a cell", () => {
  it("draws a tag, a mark, a line, an input, a button, a link, an empty note and a summary", () => {
    const remove = vi.fn();
    const press = vi.fn();
    renderUi(
      <div>
        <OptionTag color="red" onRemove={remove} removeLabel="Remove Late">
          Late
        </OptionTag>
        <OptionTag color="default">Plain</OptionTag>
        <TypeGlyph glyph="Aa" />
        <CellLine end>9</CellLine>
        <CellInput aria-label="field" defaultValue="x" />
        <CellButton onClick={press}>press</CellButton>
        <CellLink href="https://example.com">Example</CellLink>
        <CellEmpty>Empty</CellEmpty>
        <HeaderButton glyph="#" trailing={<span>↑</span>}>
          Value
        </HeaderButton>
        <SummaryButton label="Summarise" text="Sum 4" />
        <SummaryButton label="Summarise Signed" />
      </div>,
    );
    fireEvent.click(screen.getByRole("button", { name: "Remove Late" }));
    expect(remove).toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "press" }));
    expect(press).toHaveBeenCalled();
    expect(screen.getByRole("link", { name: "Example" })).toHaveAttribute(
      "rel",
      "noopener noreferrer",
    );
    expect(screen.getByRole("button", { name: "Summarise: Sum 4" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Summarise Signed" })).toBeTruthy();
  });
});

describe("the toolbar", () => {
  const view = (id: string, label: string, current: boolean, onSelect: (id: string) => void) => ({
    id,
    label,
    icon: "table" as const,
    current,
    onSelect: () => {
      onSelect(id);
    },
  });

  function Toolbar({
    onSelect,
    chips = true,
  }: {
    onSelect: (id: string) => void;
    chips?: boolean;
  }) {
    return (
      <TableToolbar
        tabsLabel="Views"
        panelId="panel"
        moreViewsLabel="More views"
        tabs={[
          view("a", "All", true, onSelect),
          view("b", "Board", false, onSelect),
          view("c", "Mine", false, onSelect),
        ]}
        trailing={<button type="button">+</button>}
        chips={
          chips ? (
            <>
              <FilterChip removeLabel="Remove one" onRemove={vi.fn()} onOpen={vi.fn()}>
                Stage is not Paused
              </FilterChip>
              <FilterChip removeLabel="Remove two" onRemove={vi.fn()}>
                Plain chip
              </FilterChip>
            </>
          ) : undefined
        }
        actions={
          <>
            <ToolbarButton icon="filter" label="Filter" count={2} />
            <ToolbarButton icon="sort" label="Sort" count={0} />
            <ToolbarButton icon="plus" label="New" primary shortcut="undo" />
          </>
        }
      />
    );
  }

  it("moves between tabs with the arrows and selects them", () => {
    const onSelect = vi.fn();
    renderUi(<Toolbar onSelect={onSelect} />);
    const all = screen.getByRole("tab", { name: "All" });
    expect(all).toHaveAttribute("aria-controls", "panel");
    all.focus();
    fireEvent.keyDown(screen.getByRole("tablist"), { key: "ArrowRight" });
    expect(onSelect).toHaveBeenLastCalledWith("b");
    fireEvent.keyDown(screen.getByRole("tablist"), { key: "ArrowLeft" });
    expect(onSelect).toHaveBeenLastCalledWith("a");
    fireEvent.keyDown(screen.getByRole("tablist"), { key: "x" });
    // Focus outside the tabs does nothing.
    (document.activeElement as HTMLElement).blur();
    fireEvent.keyDown(screen.getByRole("tablist"), { key: "ArrowRight" });
  });

  it("shows the buttons with a count, and a chip that opens or goes", () => {
    renderUi(<Toolbar onSelect={vi.fn()} />);
    expect(screen.getByRole("button", { name: "Filter (2)" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Sort" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Stage is not Paused", hidden: true })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Remove two", hidden: true })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "More views" })).toBeNull();
  });

  /** Lays the row out at a width: tabs 120, the "+" box 80, the actions 60, the chips 200, "More views" 40. */
  function atWidth(width: number) {
    const parts = vi
      .spyOn(HTMLElement.prototype, "offsetWidth", "get")
      .mockImplementation(function (this: HTMLElement) {
        if (this.dataset["tab"] !== undefined) {
          return 120;
        }
        const part = this.dataset["part"];
        return part === "rest"
          ? 80
          : part === "chips"
            ? 200
            : part === "more"
              ? 40
              : part === "actions"
                ? 60
                : 0;
      });
    const scroll = vi
      .spyOn(HTMLElement.prototype, "scrollWidth", "get")
      .mockImplementation(function (this: HTMLElement) {
        return this.dataset["part"] === "chips" ? 200 : this.offsetWidth;
      });
    const client = vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(width);
    return () => {
      parts.mockRestore();
      scroll.mockRestore();
      client.mockRestore();
    };
  }

  it("drops the chips first when the row is tight, and the Filter button keeps the count", () => {
    const restore = atWidth(500);
    const { unmount } = renderUi(<Toolbar onSelect={vi.fn()} />);
    expect(screen.queryByText("Stage is not Paused")).toBeNull();
    expect(screen.getByRole("button", { name: "Filter (2)" })).toBeTruthy();
    expect(screen.getAllByRole("tab")).toHaveLength(3);
    unmount();
    restore();
  });

  it("then folds the tabs that do not fit into a More views menu, keeping the current one", async () => {
    const restore = atWidth(300);
    const onSelect = vi.fn();
    renderUi(<Toolbar onSelect={onSelect} />);
    expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual(["All"]);
    fireEvent.click(screen.getByRole("button", { name: "More views" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Board" }));
    expect(onSelect).toHaveBeenCalledWith("b");
    expect(screen.getByRole("button", { name: "+" })).toBeTruthy();
    restore();
  });

  it("keeps the tabs it can and shows everything again when there is room", () => {
    const restore = atWidth(420);
    const { unmount } = renderUi(<Toolbar onSelect={vi.fn()} chips={false} />);
    // 80 + 40 for the menu leaves room for two tabs.
    expect(screen.getAllByRole("tab")).toHaveLength(2);
    unmount();
    restore();
    const again = atWidth(2000);
    renderUi(<Toolbar onSelect={vi.fn()} />);
    expect(screen.getAllByRole("tab")).toHaveLength(3);
    again();
  });
});

describe("the row peek", () => {
  it("opens with its properties, closes with Escape and the Close button, and trashes", () => {
    const onClose = vi.fn();
    const onTrash = vi.fn();
    const onOpen = vi.fn();
    renderUi(
      <RowPeek
        label="Row"
        crumb="Clients · row"
        openAsPage="Open as page"
        onOpenAsPage={onOpen}
        closeLabel="Close"
        onClose={onClose}
        trashLabel="Move to trash"
        onTrash={onTrash}
      >
        <PropertyList>
          <PropertyLine name="Stage" glyph={<TypeGlyph glyph="◎" />}>
            Kickoff
          </PropertyLine>
        </PropertyList>
      </RowPeek>,
    );
    const peek = screen.getByRole("dialog", { name: "Row" });
    expect(within(peek).getByRole("group", { name: "Stage" })).toBeTruthy();
    fireEvent.click(within(peek).getByRole("button", { name: "Open as page" }));
    expect(onOpen).toHaveBeenCalled();
    fireEvent.click(within(peek).getByRole("button", { name: "Move to trash" }));
    expect(onTrash).toHaveBeenCalled();
    fireEvent.keyDown(peek, { key: "a" });
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.keyDown(peek, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
    fireEvent.click(within(peek).getByRole("button", { name: "Close" }));
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it("lets a menu inside take Escape first", () => {
    const onClose = vi.fn();
    renderUi(
      <RowPeek
        label="Row"
        crumb="c"
        openAsPage="Open"
        onOpenAsPage={vi.fn()}
        closeLabel="Close"
        onClose={onClose}
      >
        <input
          aria-label="inner"
          ref={(element) => {
            // A menu inside takes Escape for itself, in the browser's own listener.
            element?.addEventListener("keydown", (event) => {
              event.preventDefault();
            });
          }}
        />
      </RowPeek>,
    );
    fireEvent.keyDown(screen.getByLabelText("inner"), { key: "Escape" });
    expect(onClose).not.toHaveBeenCalled();
  });
});

describe("the board", () => {
  const groups: BoardGroup[] = [
    {
      id: "a",
      name: "Lead",
      color: "gray",
      count: 2,
      summary: "€2,000",
      cards: [{ id: "c1", title: "Harbour", body: <span>body</span>, onOpen: vi.fn() }],
    },
    { id: "b", name: "Active", color: "green", count: 0, cards: [] },
  ];

  it("moves a card with its menu, and not into the column it is in", () => {
    const onMove = vi.fn();
    renderUi(
      <Board
        label="By stage"
        groups={groups}
        onMove={onMove}
        moveLabel={(title) => `Move ${title} to`}
        moveText="Move to"
        more={<span>1 more</span>}
        onAdd={vi.fn()}
        addLabel={(group) => `Add to ${group}`}
      />,
    );
    expect(screen.getByText("1 more")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Move Harbour to" }));
    fireEvent.click(screen.getByRole("menuitemradio", { name: "Lead" }));
    expect(onMove).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Move Harbour to" }));
    fireEvent.click(screen.getByRole("menuitemradio", { name: "Active" }));
    expect(onMove).toHaveBeenCalledWith("c1", "b");
  });

  it("moves a card by dragging it over another column", () => {
    const onMove = vi.fn();
    renderUi(
      <Board
        label="By stage"
        groups={groups}
        onMove={onMove}
        moveLabel={(t) => `Move ${t}`}
        moveText="Move"
      />,
    );
    const store = new Map<string, string>();
    const dataTransfer = {
      effectAllowed: "",
      setData: (type: string, value: string) => {
        store.set(type, value);
      },
      getData: (type: string) => store.get(type) ?? "",
    };
    const card = must(screen.getByRole("button", { name: /^Harbour/u }).parentElement);
    fireEvent.dragStart(card, { dataTransfer });
    const target = must(screen.getByRole("list", { name: "Active" }).parentElement);
    fireEvent.dragOver(target, { dataTransfer });
    fireEvent.drop(target, { dataTransfer });
    expect(onMove).toHaveBeenCalledWith("c1", "b");
    // Dropping on the column it came from, or dragging nothing, moves nothing.
    fireEvent.dragStart(card, { dataTransfer });
    fireEvent.drop(must(screen.getByRole("list", { name: "Lead" }).parentElement), {
      dataTransfer,
    });
    fireEvent.dragEnd(card, { dataTransfer });
    fireEvent.dragLeave(target);
    fireEvent.dragOver(target, { dataTransfer });
    expect(onMove).toHaveBeenCalledTimes(1);
  });

  it("is read-only for a reader: no menu, no drag, no add", () => {
    renderUi(
      <Board
        label="By stage"
        groups={groups}
        readOnly
        onMove={vi.fn()}
        moveLabel={(t) => `Move ${t}`}
        moveText="Move"
        onAdd={vi.fn()}
      />,
    );
    expect(screen.queryByRole("button", { name: /^Harbour/u })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /^Add/u })).toBeNull();
    expect(screen.getByRole("button", { name: /^Harbour/u }).parentElement).toHaveAttribute(
      "draggable",
      "false",
    );
  });
});

describe("the grid", () => {
  const columns: GridColumn[] = [
    { id: "name", width: 200, label: "Name", header: <span>Name</span> },
    { id: "value", width: 120, label: "Value", header: <span>Value</span> },
  ];
  const rows = (n: number) =>
    Array.from({ length: n }, (_, i) => ({ id: `r${String(i)}`, label: `Row ${String(i)}` }));

  function Grid({
    count = 3,
    onEdit = vi.fn(),
    onClear = vi.fn(),
    reveal,
    ...rest
  }: {
    count?: number;
    onEdit?: (position: CellPos, seed?: string) => void;
    onClear?: (position: CellPos) => void;
    reveal?: CellPos;
    hasMore?: boolean;
    onLoadMore?: () => void;
    onResize?: (columnId: string, width: number) => void;
    windowAfter?: number;
  }) {
    const [editing, setEditing] = useState<CellPos | null>(null);
    return (
      <DataGrid
        label="Clients"
        columns={columns}
        rows={rows(count)}
        editing={editing}
        reveal={reveal}
        onEdit={(position, seed) => {
          if (seed === undefined) {
            onEdit(position);
          } else {
            onEdit(position, seed);
          }
          setEditing(position);
        }}
        onClear={onClear}
        renderCell={(row, column, state) =>
          state.editing ? (
            <input
              aria-label="editor"
              onBlur={() => {
                setEditing(null);
              }}
            />
          ) : (
            `${row.label}/${column.label}`
          )
        }
        renderFooter={(column) => <span>foot {column.label}</span>}
        footerStart={<span>New row</span>}
        headerEnd={<button type="button">add column</button>}
        empty={<span>Nothing here</span>}
        loadMoreLabel="Load more"
        onResize={vi.fn()}
        {...rest}
      />
    );
  }

  it("says what it is, how big it is, and keeps one cell as the tab stop", () => {
    renderUi(<Grid />);
    const grid = screen.getByRole("grid", { name: "Clients" });
    expect(grid).toHaveAttribute("aria-rowcount", "4");
    expect(grid).toHaveAttribute("aria-colcount", "3");
    const cells = within(grid)
      .getAllByRole("gridcell")
      .filter((cell) => Object.hasOwn(cell.dataset, "row"));
    expect(cells.filter((cell) => cell.getAttribute("tabindex") === "0")).toHaveLength(1);
    expect(screen.getByText("foot Value")).toBeTruthy();
    expect(screen.getByText("New row")).toBeTruthy();
    expect(screen.getByRole("button", { name: "add column" })).toBeTruthy();
  });

  it("edits on Enter, F2, a double click, a typed character and a click on the active cell", () => {
    const onEdit = vi.fn();
    renderUi(<Grid onEdit={onEdit} />);
    const cell = screen.getByText("Row 0/Name");
    fireEvent.keyDown(cell, { key: "Enter" });
    expect(onEdit).toHaveBeenLastCalledWith({ rowId: "r0", columnId: "name" });
    fireEvent.blur(screen.getByLabelText("editor"));
    fireEvent.keyDown(screen.getByText("Row 0/Name"), { key: "F2" });
    fireEvent.blur(screen.getByLabelText("editor"));
    fireEvent.keyDown(screen.getByText("Row 0/Name"), { key: "q" });
    expect(onEdit).toHaveBeenLastCalledWith({ rowId: "r0", columnId: "name" }, "q");
    fireEvent.blur(screen.getByLabelText("editor"));
    fireEvent.doubleClick(screen.getByText("Row 1/Value"));
    expect(onEdit).toHaveBeenLastCalledWith({ rowId: "r1", columnId: "value" });
    fireEvent.blur(screen.getByLabelText("editor"));
    // A click selects; a second click on the active cell edits.
    fireEvent.click(screen.getByText("Row 2/Name"));
    const before = onEdit.mock.calls.length;
    fireEvent.click(screen.getByText("Row 2/Name"));
    expect(onEdit.mock.calls.length).toBe(before + 1);
    fireEvent.blur(screen.getByLabelText("editor"));
    // Ctrl and Alt combinations are not typing.
    fireEvent.keyDown(screen.getByText("Row 2/Name"), { key: "c", ctrlKey: true });
    fireEvent.keyDown(screen.getByText("Row 2/Name"), { key: "Shift" });
    expect(onEdit.mock.calls.length).toBe(before + 1);
  });

  it("clears on Delete or Backspace and ignores keys in the editor", () => {
    const onClear = vi.fn();
    renderUi(<Grid onClear={onClear} />);
    fireEvent.keyDown(screen.getByText("Row 0/Value"), { key: "Delete" });
    fireEvent.keyDown(screen.getByText("Row 0/Value"), { key: "Backspace" });
    expect(onClear).toHaveBeenCalledTimes(2);
    fireEvent.keyDown(screen.getByText("Row 0/Name"), { key: "Enter" });
    onClear.mockClear();
    fireEvent.keyDown(screen.getByLabelText("editor"), { key: "Delete" });
    expect(onClear).not.toHaveBeenCalled();
  });

  it("moves with Page Up and Page Down, Home and End, and stops at the edges", async () => {
    renderUi(<Grid count={30} />);
    const start = screen.getByText("Row 0/Name");
    fireEvent.keyDown(start, { key: "ArrowUp" });
    fireEvent.keyDown(start, { key: "ArrowLeft" });
    fireEvent.keyDown(start, { key: "PageDown" });
    await waitFor(() => {
      expect(screen.getByText("Row 10/Name").getAttribute("tabindex")).toBe("0");
    });
    fireEvent.keyDown(screen.getByText("Row 10/Name"), { key: "PageUp" });
    await waitFor(() => {
      expect(screen.getByText("Row 0/Name").getAttribute("tabindex")).toBe("0");
    });
    fireEvent.keyDown(screen.getByText("Row 0/Name"), { key: "End", ctrlKey: true });
    await waitFor(() => {
      expect(screen.getByText("Row 29/Value").getAttribute("tabindex")).toBe("0");
    });
    fireEvent.keyDown(screen.getByText("Row 29/Value"), { key: "ArrowDown" });
    fireEvent.keyDown(screen.getByText("Row 29/Value"), { key: "ArrowRight" });
  });

  it("says when there are no rows, and offers more when there are", () => {
    const onLoadMore = vi.fn();
    const { rerender } = renderUi(<Grid count={0} />);
    expect(screen.getByText("Nothing here")).toBeTruthy();
    rerender(<Grid count={2} hasMore onLoadMore={onLoadMore} />);
    fireEvent.click(screen.getByRole("button", { name: "Load more" }));
    expect(onLoadMore).toHaveBeenCalled();
  });

  it("moves the active cell to a row that was just made", async () => {
    const { rerender } = renderUi(<Grid count={2} />);
    rerender(<Grid count={3} reveal={{ rowId: "r2", columnId: "name" }} />);
    await waitFor(() => {
      expect(screen.getByText("Row 2/Name").getAttribute("tabindex")).toBe("0");
    });
  });

  it("resizes a column by dragging its edge or with the arrow keys", () => {
    const onResize = vi.fn();
    renderUi(<Grid onResize={onResize} />);
    const handle = screen.getByRole("slider", { name: "Name" });
    fireEvent.keyDown(handle, { key: "ArrowLeft" });
    expect(onResize).toHaveBeenLastCalledWith("name", 184);
    fireEvent.keyDown(handle, { key: "x" });
    Object.assign(handle, { setPointerCapture: vi.fn() });
    fireEvent.pointerDown(handle, { clientX: 100, pointerId: 1 });
    fireEvent.pointerMove(handle, { clientX: 150 });
    expect(handle).toHaveAttribute("aria-valuenow", "250");
    fireEvent.pointerUp(handle, { clientX: 150 });
    expect(onResize).toHaveBeenLastCalledWith("name", 250);
    // A drag beyond the limits stops at them.
    fireEvent.pointerDown(handle, { clientX: 100, pointerId: 1 });
    fireEvent.pointerUp(handle, { clientX: 5000 });
    expect(onResize).toHaveBeenLastCalledWith("name", 640);
    fireEvent.pointerMove(handle, { clientX: 1 });
    fireEvent.pointerUp(handle, { clientX: 1 });
  });

  it("draws only the rows in view past a number of them, once it knows how tall a row is", async () => {
    const heights = vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(40);
    const client = vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(200);
    const onLoadMore = vi.fn();
    renderUi(<Grid count={500} windowAfter={50} hasMore onLoadMore={onLoadMore} />);
    await waitFor(() => {
      expect(screen.queryByText("Row 400/Name")).toBeNull();
    });
    expect(screen.getByText("Row 0/Name")).toBeTruthy();
    const box = must(screen.getByRole("grid").parentElement);
    act(() => {
      box.scrollTop = 40 * 400;
      fireEvent.scroll(box);
    });
    await waitFor(() => {
      expect(screen.getByText("Row 405/Name")).toBeTruthy();
    });
    expect(screen.queryByText("Row 0/Name")).toBeNull();
    // Moving to a row that is not drawn scrolls to it.
    act(() => {
      box.scrollTop = 0;
      fireEvent.scroll(box);
    });
    await waitFor(() => {
      expect(screen.getByText("Row 0/Name")).toBeTruthy();
    });
    fireEvent.keyDown(screen.getByText("Row 0/Name"), { key: "End", ctrlKey: true });
    // The box scrolled to the last row; the browser would say so.
    act(() => {
      fireEvent.scroll(box);
    });
    await waitFor(() => {
      expect(screen.getByText("Row 499/Value")).toBeTruthy();
    });
    expect(onLoadMore).toHaveBeenCalled();
    heights.mockRestore();
    client.mockRestore();
  });
});
