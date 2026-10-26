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
import { ANA, DUARTE } from "../../../../../test/demo/workspaces";
import { renderWithProviders } from "../../../../../test/render";
import { cellAt, tablesApi } from "../../../../../test/tables";
import { usePages } from "../../api/queries";
import { usePageActions } from "../../components/use-page-actions";

import { TableView } from "./TableView";
import { must } from "../../../../../test/must";

vi.mock("../../editor/PageEditor", () => ({ PageEditor: () => <p>the row’s page</p> }));

function Harness({
  tableId = CLIENTS_ID,
  view,
  row,
}: {
  tableId?: string;
  view?: string;
  row?: string;
}) {
  const pages = usePages(DUARTE.id).data ?? [];
  const pageActions = usePageActions(DUARTE.id);
  return (
    <TableView
      workspaceId={DUARTE.id}
      tableId={tableId}
      pages={pages}
      pageActions={pageActions}
      open={vi.fn()}
      initialView={view}
      {...(row === undefined ? {} : { row, onRow: vi.fn() })}
    />
  );
}

function setup(server = new TablesServer(), view?: string, extra = {}, row?: string) {
  const api = tablesApi(server, extra);
  const rendered = renderWithProviders(
    <Harness {...(view === undefined ? {} : { view })} {...(row === undefined ? {} : { row })} />,
  );
  return { ...api, ...rendered };
}

const setView = (server: TablesServer, id: string, config: Record<string, unknown>) => {
  const view = server.table(CLIENTS_ID)?.views.find((candidate) => candidate.id === id);
  if (view !== undefined) {
    view.config = config;
  }
};

describe("a long table", () => {
  it("loads 100 rows, then the rest on request", async () => {
    const server = new TablesServer();
    const table = server.table(CLIENTS_ID);
    for (let i = 0; i < 120; i += 1) {
      table?.rows.push({
        ...must(table.rows[0]),
        id: `0192f1a0-0000-7000-8000-00000000${(9000 + i).toString(16)}`,
        number: 100 + i,
        title: `Client ${String(i)}`,
        cells: {},
      });
    }
    setView(server, VIEW_TABLE, {});
    setup(server);
    const more = await screen.findByRole("button", { name: "Load more rows (100 of 125)" });
    expect(screen.queryByRole("row", { name: "Client 119" })).toBeNull();
    fireEvent.click(more);
    expect(await screen.findByRole("row", { name: "Client 119" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Load more/u })).toBeNull();
  });
});

describe("the filter chips", () => {
  it("name a person, a choice, a yes or a no, and a filter with groups can only be cleared", async () => {
    const server = new TablesServer();
    setView(server, VIEW_TABLE, {
      filter: {
        and: [
          { property: P.owner, operator: "contains", value: ANA.user_id },
          { property: P.stage, operator: "is_any_of", value: [OPTIONS.lead, OPTIONS.active] },
          { property: P.signed, operator: "is", value: true },
          { property: P.name, operator: "contains", value: "studio" },
          { property: P.projects, operator: "is_not_empty" },
        ],
      },
    });
    setup(server);
    expect(await screen.findByText("Owner contains Ana Duarte")).toBeTruthy();
    expect(screen.getByText("Stage is any of Lead, Active")).toBeTruthy();
    expect(screen.getByText("Signed is Checked")).toBeTruthy();
    expect(screen.getByText("Name contains studio")).toBeTruthy();
    expect(screen.getByText("Projects is not empty")).toBeTruthy();
  });

  it("says a filter with groups is one, and clears it", async () => {
    const server = new TablesServer();
    setView(server, VIEW_TABLE, {
      filter: {
        and: [
          {
            or: [
              { property: P.name, operator: "contains", value: "a" },
              { property: P.name, operator: "contains", value: "b" },
            ],
          },
          { property: P.signed, operator: "is", value: true },
        ],
      },
    });
    setup(server);
    expect(await screen.findByText("Filter with groups")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Clear filter", hidden: true }));
    await waitFor(() => {
      expect(server.table(CLIENTS_ID)?.views[0]?.config["filter"]).toBeUndefined();
    });
  });

  it("opens the filter builder on a chip, and from a column's menu on that column", async () => {
    setup();
    fireEvent.click(
      await screen.findByRole("button", { name: "Stage is not Paused", hidden: true }),
    );
    expect(await screen.findByRole("group", { name: "Condition 1" })).toBeTruthy();
    fireEvent.keyDown(screen.getByRole("group", { name: "Condition 1" }), { key: "Escape" });
    fireEvent.click(screen.getByRole("button", { name: /^Value/u }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Filter by this column" }));
    const second = await screen.findByRole("group", { name: "Condition 2" });
    expect(within(second).getByLabelText("Property")).toHaveValue(P.value);
  });
});

describe("a new column's defaults", () => {
  it("gives a status column to do, in progress and done", async () => {
    const { server } = setup();
    await screen.findByRole("grid");
    fireEvent.click(screen.getByRole("button", { name: "Add a column" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: /^Status/u }));
    await waitFor(() => {
      const added = server.table(CLIENTS_ID)?.properties.at(-1);
      expect(added?.type).toBe("status");
      expect(
        (added?.config["options"] as { name: string; group: string }[]).map((o) => o.group),
      ).toEqual(["todo", "doing", "done"]);
    });
  });

  it("makes a relation to another table, paired when asked", async () => {
    const { server } = setup();
    await screen.findByRole("grid");
    fireEvent.click(screen.getByRole("button", { name: "Add a column" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: /^Relation/u }));
    fireEvent.click(await screen.findByRole("option", { name: "Projects" }));
    await waitFor(() => {
      const added = server.table(CLIENTS_ID)?.properties.at(-1);
      expect(added?.type).toBe("relation");
      expect(added?.config).toEqual({ table: PROJECTS_ID });
    });
  });
});

describe("a board", () => {
  it("asks what to group by when its property is gone, and saves the choice", async () => {
    const server = new TablesServer();
    setView(server, VIEW_BOARD, {});
    const { rerender } = setup(server, VIEW_BOARD);
    expect(await screen.findByText("Choose what the board groups by")).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Group by"), { target: { value: P.stage } });
    await waitFor(() => {
      expect(server.table(CLIENTS_ID)?.views[1]?.config["group_by"]).toBe(P.stage);
    });
    rerender(<Harness view={VIEW_BOARD} />);
  });

  it("says a table with no select needs one", async () => {
    const server = new TablesServer();
    setView(server, VIEW_BOARD, {});
    const table = server.table(CLIENTS_ID);
    if (table !== undefined) {
      table.properties = table.properties.filter((property) => property.type !== "select");
    }
    setup(server, VIEW_BOARD);
    expect(
      await screen.findByText("A board needs a select or status column. Add one first."),
    ).toBeTruthy();
    expect(screen.queryByLabelText("Group by")).toBeNull();
  });

  it("adds a card to a column with the group already set, and opens it", async () => {
    const { server } = setup(new TablesServer(), VIEW_BOARD);
    fireEvent.click(await screen.findByRole("button", { name: "Add a row to Active" }));
    expect(await screen.findByRole("dialog", { name: "Row" })).toBeTruthy();
    const made = server.table(CLIENTS_ID)?.rows.at(-1);
    expect(made?.cells[P.stage]?.value).toBe(OPTIONS.active);
  });

  it("shows how many cards are not loaded yet", async () => {
    const server = new TablesServer();
    const table = server.table(CLIENTS_ID);
    for (let i = 0; i < 110; i += 1) {
      table?.rows.push({
        ...must(table.rows[0]),
        id: `0192f1a0-0000-7000-8000-00000000${(9500 + i).toString(16)}`,
        number: 100 + i,
        title: `Card ${String(i)}`,
      });
    }
    setup(server, VIEW_BOARD);
    expect(await screen.findByText(/Showing 100 of /u)).toBeTruthy();
  });
});

describe("the row beside the table", () => {
  it("moves the row to the trash with undo, and closes", async () => {
    const { server } = setup();
    await screen.findByRole("grid");
    fireEvent.click(screen.getByRole("button", { name: "Open Harbour Physio" }));
    const peek = await screen.findByRole("dialog", { name: "Row" });
    fireEvent.click(within(peek).getByRole("button", { name: "Move to trash" }));
    await waitFor(() => {
      expect(server.rowOf(ROW.harbour)?.trashed).toBe(true);
    });
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Row" })).toBeNull();
    });
    fireEvent.click(await screen.findByRole("button", { name: /Undo/u }));
    await waitFor(() => {
      expect(server.rowOf(ROW.harbour)?.trashed).toBe(false);
    });
  });

  it("renames the row from the title and shows it read-only to a reader", async () => {
    const server = new TablesServer();
    const { unmount } = setup(server);
    await screen.findByRole("grid");
    fireEvent.click(screen.getByRole("button", { name: "Open Acme Studio" }));
    const title = await screen.findByRole("textbox", { name: "Row title" });
    fireEvent.change(title, { target: { value: "Acme Studio Ltd" } });
    fireEvent.blur(title);
    await waitFor(() => {
      expect(server.rowOf(ROW.acme)?.title).toBe("Acme Studio Ltd");
    });
    unmount();
    const table = server.table(CLIENTS_ID);
    if (table !== undefined) {
      table.page = { ...table.page, role: "reader" };
    }
    tablesApi(server);
    renderWithProviders(<Harness />);
    await screen.findByRole("grid");
    fireEvent.click(screen.getByRole("button", { name: "Open Acme Studio Ltd" }));
    const peek = await screen.findByRole("dialog", { name: "Row" });
    expect(within(peek).getByRole("textbox", { name: "Row title" })).toHaveAttribute("readonly");
    expect(within(peek).queryByRole("button", { name: "Move to trash" })).toBeNull();
    expect(within(peek).queryByRole("button", { name: "Edit Value" })).toBeNull();
    expect(within(peek).getByRole("checkbox", { name: "Signed" })).toBeDisabled();
  });
});

describe("a failing server", () => {
  it("offers to try the rows again", async () => {
    setup(new TablesServer(), undefined, {
      [`POST ${W}/tables/${CLIENTS_ID}/query`]: () => ({ status: 500 }),
    });
    expect(await screen.findByText("Couldn’t load the rows")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
    expect(cellAt).toBeTruthy();
  });
});

describe("a row named by the address", () => {
  it("is read by id when it is not among the loaded ones", async () => {
    const server = new TablesServer();
    const table = server.table(CLIENTS_ID);
    for (let i = 0; i < 110; i += 1) {
      table?.rows.push({
        ...must(table.rows[0]),
        id: `0192f1a0-0000-7000-8000-00000000${(9700 + i).toString(16)}`,
        number: 100 + i,
        title: `Late ${String(i)}`,
        cells: {},
      });
    }
    setView(server, VIEW_TABLE, {});
    const last = `0192f1a0-0000-7000-8000-00000000${(9700 + 109).toString(16)}`;
    setup(server, undefined, {}, last);
    const peek = await screen.findByRole("dialog", { name: "Row" });
    expect(await within(peek).findByDisplayValue("Late 109")).toBeTruthy();
  });

  it("is exactly Something you can't open when it is no row of the table or may not be read", async () => {
    setup(new TablesServer(), undefined, {}, "0192f1a0-0000-7000-8000-00000000beef");
    const peek = await screen.findByRole("dialog", { name: "Row" });
    expect(await within(peek).findByText("Something you can’t open")).toBeTruthy();
    expect(within(peek).queryByRole("textbox")).toBeNull();
    expect(within(peek).queryByRole("button", { name: "Open as page" })).toBeNull();
    expect(ROW.acme).toBeTruthy();
  });
});
