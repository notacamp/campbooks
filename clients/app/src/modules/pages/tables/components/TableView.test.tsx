import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { W } from "../../../../../test/demo/conversations";
import {
  CLIENTS_ID,
  OPTIONS,
  P,
  PROJECTS_ID,
  ROW,
  TablesServer,
  VIEW_BOARD,
  VIEW_TABLE,
} from "../../../../../test/demo/tables";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { renderWithProviders } from "../../../../../test/render";
import { cellAt, tablesApi } from "../../../../../test/tables";
import { usePages } from "../../api/queries";
import { usePageActions } from "../../components/use-page-actions";

import { TableView } from "./TableView";
import { must } from "../../../../../test/must";

// The editor needs a real browser; the row's page around it is what is tested here.
vi.mock("../../editor/PageEditor", () => ({
  PageEditor: () => <p>the row’s page</p>,
}));

const open = vi.fn();

function Harness({ tableId = CLIENTS_ID, view }: { tableId?: string; view?: string }) {
  const pages = usePages(DUARTE.id).data ?? [];
  const pageActions = usePageActions(DUARTE.id);
  return (
    <TableView
      workspaceId={DUARTE.id}
      tableId={tableId}
      pages={pages}
      pageActions={pageActions}
      open={open}
      initialView={view}
    />
  );
}

async function renderTable(server = new TablesServer(), view?: string) {
  const api = tablesApi(server);
  const rendered = renderWithProviders(<Harness {...(view === undefined ? {} : { view })} />);
  await screen.findByRole("tablist");
  if (view !== VIEW_BOARD) {
    await screen.findByRole("grid");
  }
  return { ...api, ...rendered };
}

const writes = (server: TablesServer, pattern: RegExp) =>
  server.calls.filter((call) => pattern.test(call));

describe("a table", () => {
  it("shows its views, the filter, the grid and the summaries of the first view", async () => {
    await renderTable();
    expect(screen.getByRole("tab", { name: "All clients", selected: true })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "By stage" })).toBeTruthy();
    // The view's filter (Stage is not Paused) is a chip, and old Mill is not here.
    expect(screen.getByText("Stage is not Paused")).toBeTruthy();
    expect(screen.queryByRole("row", { name: "Old Mill Dental" })).toBeNull();
    expect(screen.getByRole("row", { name: "Acme Studio" })).toBeTruthy();
    expect(within(cellAt("Acme Studio", "Stage")).getByText("Kickoff")).toBeTruthy();
    expect(within(cellAt("Acme Studio", "Value")).getByText("€9,600.00")).toBeTruthy();
    expect(within(cellAt("Acme Studio", "Owner")).getByText("Ana Duarte")).toBeTruthy();
    expect(within(cellAt("Acme Studio", "Signed")).getByRole("checkbox")).toBeChecked();
    // The footer sums the four clients that pass the filter, and counts those signed.
    const footer = screen.getByRole("button", { name: /Summary of Value: Sum/u });
    expect(footer.textContent).toContain("€28,400.00");
    expect(
      screen.getByRole("button", { name: /Summary of Signed: Checked/u }).textContent,
    ).toContain("50");
    // Threads link to the viewer's own records by name.
    expect(await screen.findByText("Walkthrough on Thursday")).toBeTruthy();
  });

  it("moves between cells with the arrows, Home and End, one tab stop for the grid", async () => {
    await renderTable();
    const first = cellAt("Acme Studio", "Name");
    expect(first.getAttribute("tabindex")).toBe("0");
    expect(cellAt("Acme Studio", "Stage").getAttribute("tabindex")).toBe("-1");
    first.focus();
    fireEvent.keyDown(first, { key: "ArrowRight" });
    await waitFor(() => {
      expect(cellAt("Acme Studio", "Stage").getAttribute("tabindex")).toBe("0");
    });
    fireEvent.keyDown(cellAt("Acme Studio", "Stage"), { key: "ArrowDown" });
    await waitFor(() => {
      expect(cellAt("Lumen Dental", "Stage").getAttribute("tabindex")).toBe("0");
    });
    fireEvent.keyDown(cellAt("Lumen Dental", "Stage"), { key: "End" });
    await waitFor(() => {
      expect(cellAt("Lumen Dental", "Projects").getAttribute("tabindex")).toBe("0");
    });
    fireEvent.keyDown(cellAt("Lumen Dental", "Projects"), { key: "Home", ctrlKey: true });
    await waitFor(() => {
      expect(cellAt("Acme Studio", "Name").getAttribute("tabindex")).toBe("0");
    });
    fireEvent.keyDown(cellAt("Acme Studio", "Name"), { key: "PageDown" });
    await waitFor(() => {
      expect(cellAt("Harbour Physio", "Name").getAttribute("tabindex")).toBe("0");
    });
  });

  it("edits a number in place: Enter, type, Enter writes it with the version and offers Undo", async () => {
    const { server } = await renderTable();
    const cell = cellAt("Acme Studio", "Value");
    fireEvent.keyDown(cell, { key: "Enter" });
    const input = await screen.findByRole("textbox", { name: "Value — Acme Studio" });
    expect(input).toHaveValue("9600");
    fireEvent.change(input, { target: { value: "10100" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(await screen.findByText("Value changed")).toBeTruthy();
    expect(server.rowOf(ROW.acme)?.cells[P.value]).toEqual({ value: "10100", version: 2 });
    await waitFor(() => {
      expect(within(cellAt("Acme Studio", "Value")).getByText("€10,100.00")).toBeTruthy();
    });
    fireEvent.click(screen.getByRole("button", { name: /Undo/u }));
    await waitFor(() => {
      expect(server.rowOf(ROW.acme)?.cells[P.value]?.value).toBe("9600");
    });
  });

  it("starts an edit by typing, cancels with Escape and ignores what is not a number", async () => {
    const { server } = await renderTable();
    const cell = cellAt("Acme Studio", "Value");
    fireEvent.keyDown(cell, { key: "7" });
    const input = await screen.findByRole("textbox", { name: "Value — Acme Studio" });
    expect(input).toHaveValue("7");
    fireEvent.change(input, { target: { value: "seven" } });
    expect(input).toHaveAttribute("aria-invalid", "true");
    // Enter does nothing while it is not a number...
    fireEvent.keyDown(input, { key: "Enter" });
    expect(screen.getByRole("textbox", { name: "Value — Acme Studio" })).toBeTruthy();
    // ...and Escape leaves the cell as it was.
    fireEvent.keyDown(input, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("textbox", { name: "Value — Acme Studio" })).toBeNull();
    });
    expect(writes(server, /PATCH .*cells/u)).toHaveLength(0);
  });

  it("says in the toast when somebody else changed the cell meanwhile, and keeps the member's value", async () => {
    const server = new TablesServer();
    const { server: same } = await renderTable(server);
    same.raceOn.add(`${ROW.acme}:${P.value}`);
    const cell = cellAt("Acme Studio", "Value");
    fireEvent.keyDown(cell, { key: "Enter" });
    const input = await screen.findByRole("textbox", { name: "Value — Acme Studio" });
    fireEvent.change(input, { target: { value: "1" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(await screen.findByText("Someone else changed Value too: yours is kept")).toBeTruthy();
    expect(same.rowOf(ROW.acme)?.cells[P.value]?.value).toBe("1");
  });

  it("toggles a checkbox with the keyboard or the mouse", async () => {
    const { server } = await renderTable();
    const cell = cellAt("Northwind Bakery", "Signed");
    fireEvent.keyDown(cell, { key: " " });
    await waitFor(() => {
      expect(server.rowOf(ROW.northwind)?.cells[P.signed]?.value).toBe(true);
    });
    fireEvent.click(within(cellAt("Northwind Bakery", "Signed")).getByRole("checkbox"));
    await waitFor(() => {
      expect(server.rowOf(ROW.northwind)?.cells[P.signed]?.value).toBe(false);
    });
  });

  it("clears a cell with Delete, but never the title", async () => {
    const { server } = await renderTable();
    fireEvent.keyDown(cellAt("Acme Studio", "Value"), { key: "Delete" });
    await waitFor(() => {
      expect(server.rowOf(ROW.acme)?.cells[P.value]).toBeUndefined();
    });
    fireEvent.keyDown(cellAt("Acme Studio", "Name"), { key: "Delete" });
    expect(server.rowOf(ROW.acme)?.title).toBe("Acme Studio");
  });

  it("renames a row from its title cell", async () => {
    const { server } = await renderTable();
    fireEvent.keyDown(cellAt("Harbour Physio", "Name"), { key: "Enter" });
    const input = await screen.findByRole("textbox", { name: "Name — Harbour Physio" });
    fireEvent.change(input, { target: { value: "Harbour Physio & Pilates" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(await screen.findByText("Title changed")).toBeTruthy();
    expect(server.rowOf(ROW.harbour)?.title).toBe("Harbour Physio & Pilates");
  });

  it("picks an option from the cell's picker, and makes a new one from what was typed", async () => {
    const { server } = await renderTable();
    fireEvent.keyDown(cellAt("Northwind Bakery", "Stage"), { key: "Enter" });
    const search = await screen.findByRole("combobox", { name: "Search options" });
    fireEvent.change(search, { target: { value: "kick" } });
    fireEvent.click(await screen.findByRole("option", { name: "Kickoff" }));
    await waitFor(() => {
      expect(server.rowOf(ROW.northwind)?.cells[P.stage]?.value).toBe(OPTIONS.kickoff);
    });
    // A name that is no option can be made, and is picked.
    fireEvent.keyDown(cellAt("Harbour Physio", "Stage"), { key: "Enter" });
    const again = await screen.findByRole("combobox", { name: "Search options" });
    fireEvent.change(again, { target: { value: "Won" } });
    fireEvent.click(await screen.findByRole("option", { name: "Create “Won”" }));
    await waitFor(() => {
      const options = server.table(CLIENTS_ID)?.properties.find((p) => p.id === P.stage)?.config[
        "options"
      ] as { id: string; name: string }[];
      const won = options.find((option) => option.name === "Won");
      expect(won).toBeTruthy();
      expect(server.rowOf(ROW.harbour)?.cells[P.stage]?.value).toBe(won?.id);
    });
  });

  it("picks people by name for a person cell", async () => {
    const { server } = await renderTable();
    fireEvent.keyDown(cellAt("Harbour Physio", "Owner"), { key: "Enter" });
    const search = await screen.findByRole("combobox", { name: "Search people" });
    fireEvent.change(search, { target: { value: "sofia" } });
    fireEvent.click(await screen.findByRole("option", { name: "Sofia Pessoa" }));
    // Closing the picker writes the choice.
    fireEvent.keyDown(search, { key: "Escape" });
    await waitFor(() => {
      expect(server.rowOf(ROW.harbour)?.cells[P.owner]?.value).toEqual([
        "0192f1a0-0000-7000-8000-0000000000a3",
        "0192f1a0-0000-7000-8000-0000000000a2",
      ]);
    });
  });

  it("makes a new row with the button, and its title is ready to type", async () => {
    const { server } = await renderTable();
    fireEvent.click(must(screen.getAllByRole("button", { name: /New row/u })[0]));
    const input = await screen.findByRole("textbox", { name: /Name — / });
    fireEvent.change(input, { target: { value: "Cedar Clinic" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => {
      expect(server.table(CLIENTS_ID)?.rows.at(-1)?.title).toBe("Cedar Clinic");
    });
    expect(await screen.findByRole("row", { name: "Cedar Clinic" })).toBeTruthy();
  });

  it("makes a new row with N R", async () => {
    const { server } = await renderTable();
    fireEvent.keyDown(document.body, { key: "n" });
    fireEvent.keyDown(document.body, { key: "r" });
    await waitFor(() => {
      expect(writes(server, /POST .*\/rows$/u)).toHaveLength(1);
    });
  });

  it("opens a row beside the table with its properties and page, and closes it with Escape", async () => {
    await renderTable();
    fireEvent.click(screen.getByRole("button", { name: "Open Acme Studio" }));
    const peek = await screen.findByRole("dialog", { name: "Row" });
    expect(within(peek).getByRole("textbox", { name: "Row title" })).toHaveValue("Acme Studio");
    expect(within(peek).getByRole("group", { name: "Stage" })).toBeTruthy();
    expect(within(peek).getByText("€9,600.00")).toBeTruthy();
    expect(await within(peek).findByText("the row’s page")).toBeTruthy();
    fireEvent.click(within(peek).getByRole("button", { name: "Open as page" }));
    expect(open).toHaveBeenCalledWith(`/w/${DUARTE.id}/pages/${ROW.acme}`);
    expect(screen.queryByRole("dialog", { name: "Row" })).toBeNull();
    // ⌘↵ on the active row opens it again; Escape closes it.
    fireEvent.keyDown(document.body, { key: "Enter", ctrlKey: true });
    const again = await screen.findByRole("dialog", { name: "Row" });
    fireEvent.keyDown(again, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Row" })).toBeNull();
    });
  });

  it("edits a property of the opened row in place", async () => {
    const { server } = await renderTable();
    fireEvent.click(screen.getByRole("button", { name: "Open Acme Studio" }));
    const peek = await screen.findByRole("dialog", { name: "Row" });
    fireEvent.click(within(peek).getByRole("button", { name: "Edit Value" }));
    const input = await within(peek).findByRole("textbox", { name: "Value — Acme Studio" });
    fireEvent.change(input, { target: { value: "11000" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => {
      expect(server.rowOf(ROW.acme)?.cells[P.value]?.value).toBe("11000");
    });
    fireEvent.click(within(peek).getByRole("checkbox", { name: "Signed" }));
    await waitFor(() => {
      expect(server.rowOf(ROW.acme)?.cells[P.signed]?.value).toBe(false);
    });
  });

  it("sorts from a column's menu and filters from the filter builder", async () => {
    const { server } = await renderTable();
    fireEvent.click(screen.getByRole("button", { name: "Value" }));
    fireEvent.click(await screen.findByRole("menuitemradio", { name: "Descending" }));
    await waitFor(() => {
      expect(server.table(CLIENTS_ID)?.views[0]?.config["sorts"]).toEqual([
        { property: P.value, direction: "desc" },
      ]);
    });
    await waitFor(() => {
      const rows = screen.getAllByRole("row").map((row) => row.getAttribute("aria-label"));
      expect(rows.indexOf("Lumen Dental")).toBeLessThan(rows.indexOf("Harbour Physio"));
    });

    fireEvent.click(screen.getByRole("button", { name: "Filter (1)" }));
    fireEvent.click(await screen.findByRole("button", { name: "Add condition" }));
    const condition = await screen.findByRole("group", { name: "Condition 2" });
    fireEvent.change(within(condition).getByLabelText("Property"), { target: { value: P.value } });
    fireEvent.change(within(condition).getByLabelText("Condition"), { target: { value: "gt" } });
    fireEvent.change(within(condition).getByLabelText("Value"), { target: { value: "5000" } });
    fireEvent.click(screen.getByRole("button", { name: "Apply" }));
    await waitFor(() => {
      expect(server.table(CLIENTS_ID)?.views[0]?.config["filter"]).toEqual({
        and: [
          { property: P.stage, operator: "is_not", value: OPTIONS.paused },
          { property: P.value, operator: "gt", value: "5000" },
        ],
      });
    });
    await waitFor(() => {
      expect(screen.queryByRole("row", { name: "Harbour Physio" })).toBeNull();
    });
    // A chip per condition; its x takes it away.
    fireEvent.click(
      await screen.findByRole("button", { name: "Remove the filter on Value", hidden: true }),
    );
    await waitFor(() => {
      expect(server.table(CLIENTS_ID)?.views[0]?.config["filter"]).toEqual({
        and: [{ property: P.stage, operator: "is_not", value: OPTIONS.paused }],
      });
    });
  });

  it("hides a column from its menu and shows it again from the view's options", async () => {
    const { server } = await renderTable();
    fireEvent.click(screen.getByRole("button", { name: "Projects" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Hide column" }));
    await waitFor(() => {
      expect(screen.queryByRole("columnheader", { name: "Projects" })).toBeNull();
    });
    expect(
      (server.table(CLIENTS_ID)?.views[0]?.config["properties"] as { visible: boolean }[]).some(
        (column) => !column.visible,
      ),
    ).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Options for All clients" }));
    fireEvent.click(await screen.findByRole("checkbox", { name: "Projects" }));
    await waitFor(() => {
      expect(screen.getByRole("columnheader", { name: "Projects" })).toBeTruthy();
    });
  });

  it("adds a column of a type from the + menu, renames it, and takes it away with Undo", async () => {
    const { server } = await renderTable();
    fireEvent.click(screen.getByRole("button", { name: "Add a column" }));
    fireEvent.change(await screen.findByLabelText("Column name"), { target: { value: "Region" } });
    fireEvent.click(screen.getByRole("menuitem", { name: /^Select/u }));
    await waitFor(() => {
      expect(server.table(CLIENTS_ID)?.properties.some((p) => p.name === "Region")).toBe(true);
    });
    expect(await screen.findByText("Column Region added")).toBeTruthy();
    expect(await screen.findByRole("columnheader", { name: "Region" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Undo/u }));
    await waitFor(() => {
      expect(server.table(CLIENTS_ID)?.properties.find((p) => p.name === "Region")?.removed).toBe(
        true,
      );
    });
  });

  it("changes a column's type, format and name from its menu", async () => {
    const { server } = await renderTable();
    fireEvent.click(screen.getByRole("button", { name: "Value" }));
    fireEvent.click(await screen.findByRole("menuitemradio", { name: "Percent" }));
    await waitFor(() => {
      expect(server.table(CLIENTS_ID)?.properties.find((p) => p.id === P.value)?.config).toEqual({
        format: "percent",
      });
    });
    fireEvent.click(screen.getByRole("button", { name: "Value" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: /Change type/u }));
    fireEvent.click(await screen.findByRole("menuitemradio", { name: /^Text/u }));
    await waitFor(() => {
      expect(server.table(CLIENTS_ID)?.properties.find((p) => p.id === P.value)?.type).toBe("text");
    });
    fireEvent.click(screen.getByRole("button", { name: "Value" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Rename" }));
    fireEvent.change(await screen.findByLabelText("Column name"), { target: { value: "Fee" } });
    fireEvent.click(screen.getByRole("button", { name: "Rename" }));
    await waitFor(() => {
      expect(server.table(CLIENTS_ID)?.properties.find((p) => p.id === P.value)?.name).toBe("Fee");
    });
  });

  it("summarises a column from the footer", async () => {
    const { server } = await renderTable();
    fireEvent.click(screen.getByRole("button", { name: /Summary of Value/u }));
    fireEvent.click(await screen.findByRole("menuitemradio", { name: "Average" }));
    await waitFor(() => {
      const summaries = server.table(CLIENTS_ID)?.views[0]?.config["summaries"] as {
        kind: string;
      }[];
      expect(summaries.map((summary) => summary.kind)).toContain("average");
    });
  });

  it("resizes a column with the arrow keys on its edge", async () => {
    const { server } = await renderTable();
    const handle = screen.getByRole("slider", { name: "Resize Value" });
    expect(handle).toHaveAttribute("aria-valuenow", "128");
    fireEvent.keyDown(handle, { key: "ArrowRight" });
    await waitFor(() => {
      const columns = server.table(CLIENTS_ID)?.views[0]?.config["properties"] as {
        property: string;
        width: number;
      }[];
      expect(columns.find((column) => column.property === P.value)?.width).toBe(144);
    });
  });

  it("adds a view, renames it and deletes it with Undo", async () => {
    const { server } = await renderTable();
    fireEvent.click(screen.getByRole("button", { name: "Add a view" }));
    fireEvent.change(await screen.findByLabelText("View name"), { target: { value: "Big ones" } });
    fireEvent.click(screen.getByRole("button", { name: "Create view" }));
    expect(await screen.findByRole("tab", { name: "Big ones", selected: true })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Options for Big ones" }));
    fireEvent.change(await screen.findByLabelText("View name"), { target: { value: "Largest" } });
    fireEvent.click(screen.getByRole("button", { name: "Rename view" }));
    await waitFor(() => {
      expect(server.table(CLIENTS_ID)?.views.some((view) => view.name === "Largest")).toBe(true);
    });
    fireEvent.click(await screen.findByRole("button", { name: "Options for Largest" }));
    fireEvent.click(await screen.findByRole("button", { name: "Delete view" }));
    await waitFor(() => {
      expect(server.table(CLIENTS_ID)?.views.some((view) => view.name === "Largest")).toBe(false);
    });
    fireEvent.click(await screen.findByRole("button", { name: /Undo/u }));
    await waitFor(() => {
      expect(server.table(CLIENTS_ID)?.views.some((view) => view.name === "Largest")).toBe(true);
    });
  });
});

describe("a board", () => {
  it("shows a column per option with its count and sum, and the cards", async () => {
    await renderTable(new TablesServer(), VIEW_BOARD);
    const kickoff = await screen.findByRole("list", { name: "Kickoff" });
    expect(within(kickoff).getByRole("button", { name: /^Acme Studio/u })).toBeTruthy();
    // Lumen Dental alone is Active: its column sums to its value (and so does its card).
    expect(screen.getAllByText("€12,000.00")).toHaveLength(2);
    // The empty group shows nothing; the filter left out the paused client.
    expect(screen.queryByText("Old Mill Dental")).toBeNull();
  });

  it("moves a card to another column with its Move to menu", async () => {
    const { server } = await renderTable(new TablesServer(), VIEW_BOARD);
    fireEvent.click(await screen.findByRole("button", { name: "Move Harbour Physio to" }));
    fireEvent.click(await screen.findByRole("menuitemradio", { name: "Active" }));
    await waitFor(() => {
      expect(server.rowOf(ROW.harbour)?.cells[P.stage]?.value).toBe(OPTIONS.active);
    });
    expect(await screen.findByText("Stage changed")).toBeTruthy();
  });

  it("opens a card as a row beside the board", async () => {
    await renderTable(new TablesServer(), VIEW_BOARD);
    fireEvent.click(await screen.findByRole("button", { name: /^Lumen Dental/u }));
    expect(await screen.findByRole("dialog", { name: "Row" })).toBeTruthy();
  });

  it("switches from the table to the board with the tabs", async () => {
    await renderTable();
    fireEvent.click(screen.getByRole("tab", { name: "By stage" }));
    expect(await screen.findByRole("list", { name: "Kickoff" })).toBeTruthy();
    fireEvent.click(screen.getByRole("tab", { name: "All clients" }));
    expect(await screen.findByRole("grid")).toBeTruthy();
  });
});

describe("a table the viewer may only read, or may not open", () => {
  it("is read-only for a reader: nothing to add or change", async () => {
    const server = new TablesServer();
    const table = server.table(CLIENTS_ID);
    if (table !== undefined) {
      table.page = { ...table.page, role: "reader" };
    }
    await renderTable(server);
    expect(screen.queryByRole("button", { name: /New row/u })).toBeNull();
    expect(screen.queryByRole("button", { name: "Add a column" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Add a view" })).toBeNull();
    expect(within(cellAt("Acme Studio", "Signed")).getByRole("checkbox")).toBeDisabled();
    fireEvent.keyDown(cellAt("Acme Studio", "Value"), { key: "Enter" });
    expect(screen.queryByRole("textbox", { name: /Value —/u })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Value" }));
    expect(screen.queryByRole("menuitem", { name: "Rename" })).toBeNull();
    expect(await screen.findByRole("menuitemradio", { name: "Ascending" })).toBeTruthy();
  });

  it("says only that it is something the viewer can't open", async () => {
    const server = new TablesServer();
    tablesApi(server, {
      [`GET ${W}/tables/${PROJECTS_ID}`]: () => ({ status: 404 }),
    });
    renderWithProviders(<Harness tableId={PROJECTS_ID} />);
    expect(await screen.findByText("Something you can’t open")).toBeTruthy();
    expect(screen.queryByRole("grid")).toBeNull();
  });

  it("offers to try again when the table fails to load", async () => {
    tablesApi(new TablesServer(), {
      [`GET ${W}/tables/${PROJECTS_ID}`]: () => ({ status: 500 }),
    });
    renderWithProviders(<Harness tableId={PROJECTS_ID} />);
    expect(await screen.findByText("Couldn’t load this table")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
  });
});

describe("the table and the view", () => {
  it("keeps the view's own id when an inline table asks for one", async () => {
    await renderTable(new TablesServer(), VIEW_TABLE);
    expect(screen.getByRole("tab", { name: "All clients", selected: true })).toBeTruthy();
  });
});
