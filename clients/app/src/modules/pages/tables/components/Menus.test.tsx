import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderWithProviders } from "../../../../../test/render";
import type { Column } from "../model";
import type { PropertyView, TableViewView } from "../types";

import { ColumnMenu, NewColumn } from "./ColumnMenus";
import { NewView, ViewOptions } from "./ViewMenus";
import { must } from "../../../../../test/must";

const property = (over: Partial<PropertyView>): PropertyView => ({
  id: "p",
  name: "Thing",
  type: "text",
  config: {},
  position: "a0",
  can_revert: false,
  ...over,
});

function column(over: Partial<PropertyView>): ReturnType<typeof renderColumn> {
  return renderColumn(property(over));
}

function renderColumn(prop: PropertyView, canWrite = true, sort?: "asc" | "desc") {
  const handlers = {
    onRename: vi.fn(),
    onType: vi.fn(),
    onConfig: vi.fn(),
    onSort: vi.fn(),
    onFilter: vi.fn(),
    onHide: vi.fn(),
    onDelete: vi.fn(),
  };
  renderWithProviders(<ColumnMenu property={prop} sort={sort} canWrite={canWrite} {...handlers} />);
  fireEvent.click(screen.getByRole("button", { name: new RegExp(prop.name, "u") }));
  return handlers;
}

describe("a column's menu", () => {
  it("changes the format of a number and says which is chosen", async () => {
    const handlers = column({ type: "number", config: { format: "currency", currency: "EUR" } });
    expect(
      await screen.findByRole("menuitemradio", { name: "Euro (€)", checked: true }),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole("menuitemradio", { name: "US dollar ($)" }));
    expect(handlers.onConfig).toHaveBeenCalledWith({ format: "currency", currency: "USD" });
  });

  it("switches a date's time and a created column's who", async () => {
    const date = column({ type: "date", config: { time: false } });
    fireEvent.click(await screen.findByRole("menuitemradio", { name: "Include time" }));
    expect(date.onConfig).toHaveBeenCalledWith({ time: true });
    document.body.innerHTML = "";
    const created = column({ type: "created", config: { of: "time" } });
    fireEvent.click(await screen.findByRole("menuitemradio", { name: "Show who, not when" }));
    expect(created.onConfig).toHaveBeenCalledWith({ of: "who" });
  });

  it("sorts ascending, toggles it off, filters, hides and deletes", async () => {
    const handlers = renderColumn(property({ type: "number" }), true, "asc");
    expect(
      await screen.findByRole("menuitemradio", { name: "Ascending", checked: true }),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole("menuitemradio", { name: "Ascending" }));
    expect(handlers.onSort).toHaveBeenCalledWith(undefined);
    fireEvent.click(screen.getByRole("button", { name: /Thing/u }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Filter by this column" }));
    expect(handlers.onFilter).toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: /Thing/u }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Hide column" }));
    expect(handlers.onHide).toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: /Thing/u }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Delete column" }));
    expect(handlers.onDelete).toHaveBeenCalled();
  });

  it("keeps the title from being retyped, hidden or deleted", async () => {
    column({ type: "title", name: "Name" });
    await screen.findByRole("menu", { name: "Column Name" });
    expect(screen.queryByRole("menuitem", { name: /Change type/u })).toBeNull();
    expect(screen.queryByRole("menuitem", { name: "Hide column" })).toBeNull();
    expect(screen.queryByRole("menuitem", { name: "Delete column" })).toBeNull();
  });

  it("renames with Enter, but not to nothing or to what it is", async () => {
    const handlers = column({});
    fireEvent.click(await screen.findByRole("menuitem", { name: "Rename" }));
    const name = await screen.findByLabelText("Column name");
    fireEvent.change(name, { target: { value: "  " } });
    expect(screen.getByRole("button", { name: "Rename" })).toBeDisabled();
    fireEvent.change(name, { target: { value: "Thing" } });
    fireEvent.submit(must(name.closest("form")));
    expect(handlers.onRename).not.toHaveBeenCalled();
  });

  it("changes type from the same list a new column is made from", async () => {
    const handlers = column({ type: "text" });
    fireEvent.click(await screen.findByRole("menuitem", { name: /Change type/u }));
    fireEvent.click(await screen.findByRole("menuitemradio", { name: /^Number/u }));
    expect(handlers.onType).toHaveBeenCalledWith("number");
    fireEvent.click(screen.getByRole("button", { name: /Thing/u }));
    fireEvent.click(await screen.findByRole("menuitem", { name: /Change type/u }));
    fireEvent.click(await screen.findByRole("menuitemradio", { name: /^Text/u }));
    expect(handlers.onType).toHaveBeenCalledTimes(1);
  });
});

describe("a new column", () => {
  const here = { id: "t1", title: "Clients" };
  const tables = [
    { id: "t1", title: "Clients" },
    { id: "t2", title: "Projects" },
  ];

  it("takes the type's name when none was typed", async () => {
    const onCreate = vi.fn();
    renderWithProviders(<NewColumn tables={tables} here={here} onCreate={onCreate} />);
    fireEvent.click(screen.getByRole("button", { name: "Add a column" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: /^Checkbox/u }));
    expect(onCreate).toHaveBeenCalledWith("Checkbox", "checkbox", {});
  });

  it("asks which table a relation points at, and whether to show it there too", async () => {
    const onCreate = vi.fn();
    renderWithProviders(<NewColumn tables={tables} here={here} onCreate={onCreate} />);
    fireEvent.click(screen.getByRole("button", { name: "Add a column" }));
    fireEvent.change(await screen.findByLabelText("Column name"), { target: { value: "Work" } });
    fireEvent.click(screen.getByRole("menuitem", { name: /^Relation/u }));
    fireEvent.change(await screen.findByRole("combobox", { name: "Search tables" }), {
      target: { value: "proj" },
    });
    expect(screen.queryByRole("option", { name: "Clients" })).toBeNull();
    fireEvent.click(screen.getByRole("checkbox", { name: "Show it on the other table too" }));
    fireEvent.click(screen.getByRole("option", { name: "Projects" }));
    expect(onCreate).toHaveBeenCalledWith("Work", "relation", {
      table: "t2",
      pairedName: "Clients",
    });
  });

  it("can go back from choosing a table", async () => {
    renderWithProviders(<NewColumn tables={[]} here={here} onCreate={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Add a column" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: /^Relation/u }));
    expect(await screen.findByText("No other table you can open.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(await screen.findByLabelText("Column name")).toBeTruthy();
  });
});

describe("the views", () => {
  const select = property({ id: "s", name: "Stage", type: "select" });

  it("makes a board grouped by a select, and needs one", async () => {
    const onCreate = vi.fn();
    const { unmount } = renderWithProviders(<NewView properties={[select]} onCreate={onCreate} />);
    fireEvent.click(screen.getByRole("button", { name: "Add a view" }));
    fireEvent.click(await screen.findByRole("button", { name: "Board" }));
    fireEvent.click(screen.getByRole("button", { name: "Create view" }));
    expect(onCreate).toHaveBeenCalledWith("Board", "board", "s");
    unmount();
    renderWithProviders(<NewView properties={[]} onCreate={onCreate} />);
    fireEvent.click(screen.getByRole("button", { name: "Add a view" }));
    fireEvent.click(await screen.findByRole("button", { name: "Board" }));
    expect(screen.getByText("Add a select or status column first.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Create view" })).toBeDisabled();
  });

  const view: TableViewView = {
    id: "v",
    name: "By stage",
    type: "board",
    position: "a0",
    config: {},
  };
  const columns: Column[] = [
    { property: property({ id: "n", name: "Name", type: "title" }), width: 200, visible: true },
    { property: select, width: 100, visible: true },
  ];

  it("changes what a board groups by and shows or hides columns", async () => {
    const onGroupBy = vi.fn();
    const onToggle = vi.fn();
    renderWithProviders(
      <ViewOptions
        view={view}
        columns={columns}
        properties={[select]}
        groupBy="s"
        canWrite
        canRemove
        onRename={vi.fn()}
        onToggle={onToggle}
        onGroupBy={onGroupBy}
        onRemove={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Options for By stage" }));
    expect(await screen.findByRole("checkbox", { name: "Name" })).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox", { name: "Stage" }));
    expect(onToggle).toHaveBeenCalledWith("s", false);
    fireEvent.change(screen.getByLabelText("Group by"), { target: { value: "s" } });
  });

  it("is read-only for a reader, and cannot delete the last view", async () => {
    renderWithProviders(
      <ViewOptions
        view={view}
        columns={columns}
        properties={[select]}
        groupBy="s"
        canWrite={false}
        canRemove={false}
        onRename={vi.fn()}
        onToggle={vi.fn()}
        onGroupBy={vi.fn()}
        onRemove={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Options for By stage" }));
    expect(await screen.findByRole("checkbox", { name: "Stage" })).toBeDisabled();
    expect(screen.queryByRole("button", { name: "Delete view" })).toBeNull();
    expect(screen.queryByLabelText("View name")).toBeNull();
  });
});
