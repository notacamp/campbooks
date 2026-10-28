import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import {
  CELL_CONFLICT,
  COLUMN_CONFLICT,
  NotionTablesServer,
  SCORE,
  type SyncedOptions,
} from "../../../../../test/demo/notion-tables";
import { CLIENTS_ID, P, ROW } from "../../../../../test/demo/tables";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { renderWithProviders } from "../../../../../test/render";
import { cellAt, tablesApi } from "../../../../../test/tables";
import { usePages } from "../../api/queries";
import { usePageActions } from "../../components/use-page-actions";

import { RowBodyContext } from "./row-body";
import { TableView } from "./TableView";

const stubBody = () => <p>the row’s page</p>;

function Harness() {
  const pages = usePages(DUARTE.id).data ?? [];
  const pageActions = usePageActions(DUARTE.id);
  return (
    <RowBodyContext value={stubBody}>
      <TableView
        workspaceId={DUARTE.id}
        tableId={CLIENTS_ID}
        pages={pages}
        pageActions={pageActions}
        open={vi.fn()}
      />
    </RowBodyContext>
  );
}

async function renderSynced(options: SyncedOptions = {}, role?: "reader") {
  const server = new NotionTablesServer();
  server.makeSynced(options);
  if (role !== undefined) {
    const table = server.table(CLIENTS_ID);
    if (table !== undefined) {
      table.page = { ...table.page, role };
    }
  }
  const api = tablesApi(server);
  renderWithProviders(<Harness />);
  await screen.findByRole("grid");
  await screen.findByText(/^Synced with Notion/u);
  return { ...api, server };
}

const header = (name: string) => screen.getByRole("button", { name: new RegExp(`^${name}`, "u") });

describe("a table kept in step with Notion", () => {
  it("says it is synced, with Open in Notion and Stop syncing", async () => {
    const { server } = await renderSynced();
    expect(screen.getByText("Synced with Notion · 1 min. ago")).toBeVisible();
    expect(screen.getByRole("link", { name: "Open in Notion" })).toHaveAttribute(
      "href",
      expect.stringContaining("notion.so/"),
    );
    fireEvent.click(screen.getByRole("button", { name: "Stop syncing" }));
    const ask = await screen.findByRole("group", { name: /Stop keeping this table in step/u });
    fireEvent.click(within(ask).getByRole("button", { name: "Stop syncing" }));
    await waitFor(() => {
      expect(server.link?.state).toBe("stopped");
    });
    // The toast and the status line both say it.
    expect((await screen.findAllByText("Stopped syncing with Notion")).length).toBe(2);
    await waitFor(() => {
      expect(screen.queryByRole("img", { name: /Campbooks only/u })).toBeNull();
    });
    // What only Notion computes stays read-only.
    expect(screen.getAllByRole("img", { name: /Read-only/u }).length).toBeGreaterThan(0);
  });

  it("shows a lock with the reason on what only Notion changes, and its cells cannot be edited", async () => {
    await renderSynced();
    expect(
      within(header("Score")).getByRole("img", {
        name: "Read-only: a formula, computed in Notion",
      }),
    ).toBeVisible();
    expect(
      within(header("Owner")).getByRole("img", { name: "Read-only: people stay Notion’s" }),
    ).toBeVisible();
    expect(
      within(header("Projects")).getByRole("img", { name: "Read-only: kept in Notion" }),
    ).toBeVisible();
    const score = cellAt("Acme Studio", "Score");
    score.focus();
    fireEvent.keyDown(score, { key: "Enter" });
    fireEvent.keyDown(score, { key: "8" });
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(within(score).getByText("82")).toBeVisible();
    // Stage is Notion's too and stays editable.
    expect(within(header("Stage")).queryByRole("img")).toBeNull();
  });

  it("marks the columns that are only Campbooks' and says so in the menu", async () => {
    await renderSynced();
    expect(
      within(header("Threads")).getByRole("img", { name: "Campbooks only: never sent to Notion" }),
    ).toBeVisible();
    fireEvent.click(header("Threads"));
    expect(await screen.findByText(/Threads is Campbooks only/u)).toBeVisible();
  });

  it("lets a Notion formula only be renamed or removed, which go to Notion too", async () => {
    await renderSynced();
    fireEvent.click(header("Score"));
    expect(await screen.findByText(/Score is a Notion formula/u)).toBeVisible();
    expect(screen.queryByRole("menuitem", { name: /Change type/u })).toBeNull();
    expect(screen.getByRole("menuitem", { name: "Rename" })).toBeVisible();
  });

  it("says which types stay in Campbooks when a column's type is chosen", async () => {
    await renderSynced();
    fireEvent.click(screen.getByRole("button", { name: "Add a column" }));
    const status = await screen.findByRole("menuitem", { name: /^Status/u });
    expect(within(status).getByText("Stays in Campbooks")).toBeVisible();
    const text = screen.getByRole("menuitem", { name: /^Text/u });
    expect(within(text).queryByText("Stays in Campbooks")).toBeNull();
  });

  it("an ordinary table has no marks, no badge and no extra notes", async () => {
    const server = new NotionTablesServer();
    tablesApi(server);
    renderWithProviders(<Harness />);
    await screen.findByRole("grid");
    expect(screen.queryByText(/Synced with Notion/u)).toBeNull();
    expect(screen.queryByRole("img", { name: /Campbooks only/u })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Add a column" }));
    const status = await screen.findByRole("menuitem", { name: /^Status/u });
    expect(within(status).queryByText("Stays in Campbooks")).toBeNull();
  });
});

describe("a cell changed in both places", () => {
  it("shows Notion's value under ours in the cell's popover and keeps Notion's with Undo", async () => {
    const { server, mocked } = await renderSynced({ cellConflict: true });
    expect(screen.getByRole("button", { name: "1 change to look at" })).toBeVisible();
    fireEvent.click(
      within(cellAt("Northwind Bakery", "Stage")).getByRole("button", {
        name: "Stage changed in Notion too: choose a version",
      }),
    );
    const card = await screen.findByRole("region", { name: "A change from Notion in Stage" });
    expect(within(card).getByText("Kickoff")).toBeVisible();
    expect(within(card).getByText("Proposal sent")).toBeVisible();
    // Neither has gone anywhere yet, and the cell cannot be edited meanwhile.
    fireEvent.click(within(card).getByRole("button", { name: "Use Notion’s for Stage" }));
    await waitFor(() => {
      expect(
        mocked.calls.some(
          (call) =>
            call.route.endsWith(`/notion/conflicts/${CELL_CONFLICT}`) &&
            (call.body as { keep: string }).keep === "theirs",
        ),
      ).toBe(true);
    });
    await waitFor(() => {
      expect(within(cellAt("Northwind Bakery", "Stage")).getByText("Proposal sent")).toBeVisible();
    });
    expect(await screen.findByText(/Using Notion’s version/u)).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /Undo/u }));
    await waitFor(() => {
      expect(server.conflicts).toHaveLength(1);
    });
    await waitFor(() => {
      expect(within(cellAt("Northwind Bakery", "Stage")).getByText("Kickoff")).toBeVisible();
    });
  });

  it("is shown in the row beside the table, where Keep mine settles it", async () => {
    const { server } = await renderSynced({ cellConflict: true });
    fireEvent.click(screen.getByRole("button", { name: "Open Northwind Bakery" }));
    const peek = await screen.findByRole("dialog", { name: "Row" });
    const card = await within(peek).findByRole("region", { name: "A change from Notion in Stage" });
    expect(within(card).getByText(/Stage was changed here and in Notion/u)).toBeVisible();
    fireEvent.click(within(card).getByRole("button", { name: "Keep mine for Stage" }));
    await waitFor(() => {
      expect(server.conflicts).toHaveLength(0);
    });
    await waitFor(() => {
      expect(
        within(peek).queryByRole("region", { name: "A change from Notion in Stage" }),
      ).toBeNull();
    });
    expect(server.rowOf(ROW.northwind)?.cells[P.stage]?.value).toBe("o-kickoff");
  });

  it("jumps to the first change from the badge", async () => {
    await renderSynced({ cellConflict: true });
    fireEvent.click(screen.getByRole("button", { name: "1 change to look at" }));
    const peek = await screen.findByRole("dialog", { name: "Row" });
    expect(
      await within(peek).findByRole("region", { name: "A change from Notion in Stage" }),
    ).toBeVisible();
  });

  it("is only shown, not settled, to someone who can only read the table", async () => {
    await renderSynced({ cellConflict: true }, "reader");
    fireEvent.click(
      within(cellAt("Northwind Bakery", "Stage")).getByRole("button", {
        name: "Stage changed in Notion too: choose a version",
      }),
    );
    const card = await screen.findByRole("region", { name: "A change from Notion in Stage" });
    expect(within(card).getByText("Proposal sent")).toBeVisible();
    expect(within(card).queryByRole("button")).toBeNull();
  });
});

describe("a column changed in both places", () => {
  it("shows both names in a banner above the table and uses Notion's with Undo", async () => {
    const { server, mocked } = await renderSynced({ columnConflict: true });
    const banner = await screen.findByRole("region", { name: "Columns changed in both places" });
    expect(within(banner).getByText(/“Deal size” was changed here and in Notion/u)).toBeVisible();
    expect(
      within(banner).getByText("Name: “Deal size” here, “Contract value” in Notion"),
    ).toBeVisible();
    expect(within(banner).getByRole("button", { name: "Keep “Deal size”" })).toBeVisible();
    fireEvent.click(within(banner).getByRole("button", { name: "Use “Contract value”" }));
    await waitFor(() => {
      expect(
        mocked.calls.some((call) => call.route.endsWith(`/notion/conflicts/${COLUMN_CONFLICT}`)),
      ).toBe(true);
    });
    expect(await screen.findByRole("columnheader", { name: "Contract value" })).toBeVisible();
    expect(screen.queryByRole("region", { name: "Columns changed in both places" })).toBeNull();
    fireEvent.click(await screen.findByRole("button", { name: /Undo/u }));
    await waitFor(() => {
      expect(server.conflicts).toHaveLength(1);
    });
    expect(await screen.findByRole("columnheader", { name: "Deal size" })).toBeVisible();
  });
});

describe("removing a column of a synced table", () => {
  it("asks 'Remove from both places?' and sends nothing until confirmed", async () => {
    const { server } = await renderSynced();
    fireEvent.click(header("Score"));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Remove from both places…" }));
    const dialog = await screen.findByRole("dialog", { name: "Remove “Score” from both places?" });
    expect(within(dialog).getByText(/5 rows/u)).toBeVisible();
    expect(within(dialog).getByText(/Campbooks keeps them for 30 days/u)).toBeVisible();
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: /Remove “Score”/u })).toBeNull();
    });
    expect(server.calls.some((call) => call.startsWith("DELETE"))).toBe(false);
    expect(server.confirmedRemovals).toEqual([]);
  });

  it("removes it in both places on confirm, and Undo restores it", async () => {
    const { server } = await renderSynced();
    fireEvent.click(header("Score"));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Remove from both places…" }));
    const dialog = await screen.findByRole("dialog", { name: "Remove “Score” from both places?" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Remove from both" }));
    await waitFor(() => {
      expect(server.confirmedRemovals).toEqual([SCORE]);
    });
    expect(await screen.findByText("Column Score removed from both places")).toBeVisible();
    await waitFor(() => {
      expect(screen.queryByRole("columnheader", { name: "Score" })).toBeNull();
    });
    fireEvent.click(screen.getByRole("button", { name: /Undo/u }));
    await waitFor(() => {
      expect(server.calls.some((call) => call.endsWith(`/properties/${SCORE}/restore`))).toBe(true);
    });
    expect(await screen.findByRole("columnheader", { name: "Score" })).toBeVisible();
  });

  it("removes a Campbooks-only column at once, without asking about Notion", async () => {
    const { server } = await renderSynced();
    fireEvent.click(header("Threads"));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Delete column" }));
    await waitFor(() => {
      expect(screen.queryByRole("columnheader", { name: "Threads" })).toBeNull();
    });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(server.confirmedRemovals).toEqual([]);
  });

  it("tries a status column plain first, and asks only when the server says it is Notion's too", async () => {
    const server = new NotionTablesServer();
    server.makeSynced();
    server.table(CLIENTS_ID)?.properties.push({
      id: "0192f1a0-0000-7000-8000-000000009999",
      name: "Health",
      type: "status",
      config: { options: [] },
      position: "a99",
      can_revert: false,
    });
    tablesApi(server);
    renderWithProviders(<Harness />);
    await screen.findByRole("grid");
    await screen.findByText(/^Synced with Notion/u);
    fireEvent.click(header("Health"));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Delete column" }));
    const dialog = await screen.findByRole("dialog", { name: "Remove “Health” from both places?" });
    expect(server.calls).toContain(
      `DELETE /tables/${CLIENTS_ID}/properties/0192f1a0-0000-7000-8000-000000009999 (refused)`,
    );
    expect(server.confirmedRemovals).toEqual([]);
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
  });

  it("says so when the removal could not be made, and changes nothing", async () => {
    const { server } = await renderSynced();
    fireEvent.click(header("Score"));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Remove from both places…" }));
    const dialog = await screen.findByRole("dialog", { name: "Remove “Score” from both places?" });
    server.table(CLIENTS_ID)?.properties.splice(0, server.table(CLIENTS_ID)?.properties.length);
    fireEvent.click(within(dialog).getByRole("button", { name: "Remove from both" }));
    expect(
      await within(dialog).findByText("Couldn’t remove it. Nothing was changed."),
    ).toBeVisible();
  });
});

describe("the options of a synced select", () => {
  it("says renaming, recolouring and removing stay in Campbooks", async () => {
    await renderSynced();
    const cell = cellAt("Acme Studio", "Stage");
    cell.focus();
    fireEvent.keyDown(cell, { key: "Enter" });
    fireEvent.click(await screen.findByRole("button", { name: "Edit option Active" }));
    expect(
      await screen.findByText(/Renaming, recolouring or removing one stays in Campbooks/u),
    ).toBeVisible();
  });
});
