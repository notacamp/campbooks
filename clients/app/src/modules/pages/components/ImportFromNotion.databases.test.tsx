import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { W } from "../../../../test/demo/conversations";
import { NOTION_DATABASES } from "../../../../test/demo/notion-tables";
import { NOTION_PAGES } from "../../../../test/demo/pages";
import { TEAM } from "../../../../test/demo/tasks";
import { DUARTE } from "../../../../test/demo/workspaces";
import { type MockHandler, mockApi, renderWithProviders } from "../../../../test/render";
import type { NotionSource, UseNotionSource } from "../notion-source";

import { ImportFromNotion } from "./ImportFromNotion";

const CONNECTION = { id: "c1", name: "Acme", needsReauth: false };

function source(
  databases: NotionSource["databases"],
  pages: NotionSource["list"]["pages"] = NOTION_PAGES,
): UseNotionSource {
  return () => ({
    connections: [CONNECTION],
    list: {
      status: "ready",
      pages,
      truncated: false,
      error: undefined,
      retry: vi.fn(),
    },
    ...(databases === undefined ? {} : { databases }),
  });
}

const ready = (items = NOTION_DATABASES): NotionSource["databases"] => ({
  status: "ready",
  items,
  truncated: false,
  retry: vi.fn(),
});

function renderImport(useSource: UseNotionSource, extra: Record<string, MockHandler> = {}) {
  const mocked = mockApi({
    [`GET ${W}/members`]: { body: { members: TEAM } },
    [`GET ${W}/pages`]: { body: { pages: [] } },
    [`GET ${W}/pages/notion/links`]: { body: { links: [] } },
    [`POST ${W}/pages/notion/imports`]: {
      status: 201,
      body: {
        pages: [],
        skipped: 0,
        tables: [{ data_source: NOTION_DATABASES[0]?.data_source, table: "table-1" }],
      },
    },
    ...extra,
  });
  const onImported = vi.fn();
  renderWithProviders(
    <ImportFromNotion
      workspaceId={DUARTE.id}
      open
      onClose={vi.fn()}
      useSource={useSource}
      settingsHref="/w/x/settings/mailboxes"
      onImported={onImported}
    />,
  );
  return { mocked, onImported };
}

describe("ImportFromNotion with Notion databases", () => {
  it("lists the databases beside the pages, each with its columns", async () => {
    renderImport(source(ready()));
    const dialog = await screen.findByRole("dialog", { name: "Import from Notion" });
    const group = within(dialog).getByRole("group", { name: "Databases from Notion" });
    expect(within(group).getByLabelText("Clients")).toBeVisible();
    expect(within(group).getByText("7 columns")).toBeVisible();
    expect(within(dialog).getByRole("group", { name: "Pages from Notion" })).toBeVisible();
    expect(
      within(dialog).getByText(/Databases become tables, and each entry becomes a row/u),
    ).toBeVisible();
    expect(within(dialog).getByText(/Formulas, rollups and people stay Notion’s/u)).toBeVisible();
    expect(within(dialog).getByRole("button", { name: "Import" })).toBeDisabled();
  });

  it("imports a ticked database as a table and opens it", async () => {
    const { mocked, onImported } = renderImport(source(ready()));
    const dialog = await screen.findByRole("dialog", { name: "Import from Notion" });
    fireEvent.click(within(dialog).getByLabelText("Clients"));
    fireEvent.click(within(dialog).getByRole("button", { name: "Import 1 database" }));
    await waitFor(() => {
      expect(onImported).toHaveBeenCalledWith("table-1");
    });
    expect(mocked.calls.find((c) => c.route === `POST ${W}/pages/notion/imports`)?.body).toEqual({
      connection: "c1",
      pages: [],
      data_sources: [{ data_source: NOTION_DATABASES[0]?.data_source, title: "Clients" }],
    });
    expect(await screen.findByText("Imported 1 table from Notion")).toBeVisible();
  });

  it("names pages and databases together on the button, and in the toast", async () => {
    const { mocked } = renderImport(source(ready()), {
      [`POST ${W}/pages/notion/imports`]: {
        status: 201,
        body: {
          pages: [{ page: "page-1", notion_page: NOTION_PAGES[3]?.notion_page }],
          skipped: 0,
          tables: [{ data_source: NOTION_DATABASES[0]?.data_source, table: "table-1" }],
        },
      },
    });
    const dialog = await screen.findByRole("dialog", { name: "Import from Notion" });
    fireEvent.click(within(dialog).getByLabelText("Meeting notes"));
    fireEvent.click(within(dialog).getByLabelText("Clients"));
    fireEvent.click(within(dialog).getByRole("button", { name: "Import 2 items" }));
    await waitFor(() => {
      expect(mocked.calls.some((c) => c.route === `POST ${W}/pages/notion/imports`)).toBe(true);
    });
    expect(await screen.findByText("Imported 1 page and 1 table from Notion")).toBeVisible();
  });

  it("shows a database that is already a table as synced, and does not send it again", async () => {
    renderImport(
      source(ready(NOTION_DATABASES.map((database) => ({ ...database, linked: true })))),
    );
    const dialog = await screen.findByRole("dialog", { name: "Import from Notion" });
    const clients = within(dialog).getByLabelText("Clients");
    expect(clients).toBeDisabled();
    expect(clients).toBeChecked();
    expect(within(dialog).getByRole("button", { name: "Import" })).toBeDisabled();
  });

  it("offers databases even when no page is shared", async () => {
    renderImport(source(ready(), []));
    const dialog = await screen.findByRole("dialog", { name: "Import from Notion" });
    expect(within(dialog).getByLabelText("Projects")).toBeVisible();
    expect(within(dialog).queryByText("No pages shared yet")).toBeNull();
  });

  it("counts databases against the most one import takes", async () => {
    const many = Array.from({ length: 101 }, (_, n) => ({
      data_source: `d${String(n)}`,
      title: `Database ${String(n)}`,
      edited_at: "2026-10-27T09:00:00Z",
      columns: 2,
      linked: false,
    }));
    renderImport(source(ready(many), []));
    const dialog = await screen.findByRole("dialog", { name: "Import from Notion" });
    for (const box of within(dialog).getAllByRole("checkbox")) {
      fireEvent.click(box);
    }
    expect(within(dialog).getByRole("alert")).toHaveTextContent(
      "Pick at most 100 pages and databases at a time.",
    );
    expect(within(dialog).getByRole("button", { name: "Import 101 databases" })).toBeDisabled();
  });

  it("says when the databases could not be listed, and tries again", async () => {
    const retry = vi.fn();
    renderImport(source({ status: "error", items: [], truncated: false, retry }));
    const dialog = await screen.findByRole("dialog", { name: "Import from Notion" });
    expect(within(dialog).getByText(/Databases couldn’t be listed just now/u)).toBeVisible();
    // Pages still list and import.
    expect(within(dialog).getByLabelText("Meeting notes")).toBeVisible();
    fireEvent.click(within(dialog).getByRole("button", { name: "Try again" }));
    expect(retry).toHaveBeenCalled();
  });

  it("says the databases listed are cut, when they are", async () => {
    renderImport(
      source({ status: "ready", items: NOTION_DATABASES, truncated: true, retry: vi.fn() }),
    );
    expect(
      await screen.findByText(/Notion lists more databases than are shown here/u),
    ).toBeVisible();
  });

  it("offers pages only where there are no databases to offer", async () => {
    renderImport(source({ status: "off", items: [], truncated: false, retry: vi.fn() }));
    const dialog = await screen.findByRole("dialog", { name: "Import from Notion" });
    expect(within(dialog).queryByRole("group", { name: "Databases from Notion" })).toBeNull();
    expect(within(dialog).getByRole("button", { name: "Import pages" })).toBeDisabled();
    expect(within(dialog).getByText(/These are the pages you shared/u)).toBeVisible();
  });
});
