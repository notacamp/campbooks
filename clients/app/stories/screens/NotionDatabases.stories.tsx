import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, waitFor, within } from "storybook/test";

import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import { COUNTS, MARTA, MARTA_DETAIL } from "../../test/demo/attention";
import { INBOX, STREAM_COUNTS, W } from "../../test/demo/conversations";
import { CONNECTION, NOTION_CONNECTION } from "../../test/demo/identity";
import { NotionTablesServer, type SyncedOptions } from "../../test/demo/notion-tables";
import { PAGES } from "../../test/demo/pages";
import { CLIENTS_ID, ROW, type Handler } from "../../test/demo/tables";
import { LEASE, TEAM } from "../../test/demo/tasks";
import { DUARTE, SESSION } from "../../test/demo/workspaces";

import { mockFetch, saveThemeFromGlobals } from "./mock-fetch";

// Notion databases as tables (campbooks-cloud#377) in the real app against a
// small in-memory fake Notion: the import with databases beside pages, the
// clients table kept in step (locked columns, the badge, Campbooks-only
// marks), a cell and a column changed in both places, and removing a column
// from both places. The server keeps its state and starts over each time the
// app loads. Each scenario has a story that plays it at desktop and phone
// width, asserting that the controls it needs are on screen and can be
// clicked (nothing sits over them), and an `*Idle` twin with no `play` that a
// recording drives.

const PATH = `/w/${DUARTE.id}/pages`;
const TABLE = `${PATH}/${CLIENTS_ID}`;
const PEEK = `${TABLE}?row=${ROW.northwind}`;
const IMPORT = `${PATH}?import=notion`;
const server = new NotionTablesServer();

/** The workspace around the table: members, the tree and the Notion connection. */
function api(prepare: (held: NotionTablesServer) => void): Record<string, Handler> {
  const shell: Record<string, Handler> = {
    // The app loading starts the server over.
    "GET /bff/session": () => {
      server.reset();
      prepare(server);
      return { body: SESSION };
    },
    "GET /bff/api/v1/workspaces": () => ({ body: { workspaces: [DUARTE] } }),
    [`GET ${W}/connections`]: () => ({ body: { connections: [CONNECTION, NOTION_CONNECTION] } }),
    [`GET ${W}/inboxes`]: () => ({ body: { inboxes: [INBOX] } }),
    [`GET ${W}/streams`]: () => ({ body: { streams: STREAM_COUNTS } }),
    [`GET ${W}/members`]: () => ({ body: { members: TEAM } }),
    [`GET ${W}/conversations`]: () => ({ body: { conversations: [MARTA], next_cursor: null } }),
    [`GET ${W}/conversations/${MARTA.id}`]: () => ({ body: MARTA_DETAIL }),
    [`GET ${W}/tasks`]: () => ({ body: { tasks: [] } }),
    [`GET ${W}/tasks/${LEASE.id}`]: () => ({ body: { task: LEASE } }),
    [`GET ${W}/correspondents`]: () => ({ body: { correspondents: [] } }),
    [`GET ${W}/search`]: () => ({ body: { hits: [] } }),
    [`GET ${W}/attention`]: () => ({ body: { items: [], counts: COUNTS } }),
    [`GET ${W}/pages`]: () => ({
      body: { pages: [...PAGES, ...server.pages()], tables: true },
    }),
    [`GET ${W}/pages/backlinks`]: () => ({ body: { pages: [] } }),
  };
  return server.routes(W, shell);
}

function AppAt({ path }: { path: string }) {
  const [app] = useState(() => {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const router = createAppRouter(queryClient, createMemoryHistory({ initialEntries: [path] }));
    return { queryClient, router };
  });
  return (
    <QueryClientProvider client={app.queryClient}>
      <UndoProvider>
        <RouterProvider router={app.router} />
      </UndoProvider>
    </QueryClientProvider>
  );
}

interface ScreenArgs {
  path: string;
  /** How the clients table starts: kept in step with Notion, with these conflicts. */
  synced?: SyncedOptions;
}

const meta = {
  title: "Screens/Notion databases",
  parameters: { layout: "fullscreen" },
  beforeEach: ({ args, globals }) => {
    saveThemeFromGlobals(globals);
    return mockFetch(
      api((held) => {
        held.makeSynced(args.synced);
      }),
    );
  },
  render: ({ path }) => <AppAt path={path} />,
} satisfies Meta<ScreenArgs>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A viewport the story test runs at (the addon reads `globals.viewport`). */
function at(name: string, width: number, height: number) {
  return {
    parameters: {
      viewport: {
        options: {
          [name]: { name, styles: { width: `${String(width)}px`, height: `${String(height)}px` } },
        },
      },
    },
    globals: { viewport: { value: name } },
  };
}
const DESKTOP = at("desktop", 1440, 900);
const PHONE = at("phone", 375, 812);

/**
 * Whether a click on the element would land on it: it has a size, it is on
 * screen sideways once scrolled to, and nothing sits over its middle.
 */
function clickable(element: HTMLElement): boolean {
  const target =
    element instanceof HTMLInputElement && element.labels?.[0] instanceof HTMLElement
      ? element.labels[0]
      : element;
  target.scrollIntoView({ block: "nearest", inline: "nearest" });
  const box = target.getBoundingClientRect();
  const view = target.ownerDocument.defaultView;
  if (box.width === 0 || box.height === 0 || view === null) {
    return false;
  }
  if (box.left < -1 || box.right > view.innerWidth + 1) {
    return false;
  }
  const hit = target.ownerDocument.elementFromPoint(
    box.left + box.width / 2,
    box.top + box.height / 2,
  );
  return hit !== null && target.contains(hit);
}

/** Nothing in the row sits over anything else (a row that wraps puts its children on lines of their own). */
function noOverlap(row: HTMLElement): boolean {
  const boxes = [...row.children]
    .filter((child): child is HTMLElement => child instanceof HTMLElement)
    .map((child) => child.getBoundingClientRect())
    .filter((box) => box.width > 0 && box.height > 0);
  return boxes.every((box, index) =>
    boxes
      .slice(index + 1)
      .every(
        (other) =>
          !(
            box.left < other.right - 1 &&
            other.left < box.right - 1 &&
            box.top < other.bottom - 1 &&
            other.top < box.bottom - 1
          ),
      ),
  );
}

const body = (canvasElement: HTMLElement) => within(canvasElement.ownerDocument.body);

/** The page that holds the sync's controls: Open in Notion, Stop syncing and the changes to look at. */
async function syncRowFits(canvasElement: HTMLElement) {
  const canvas = within(canvasElement);
  await canvas.findByRole("grid");
  const status = await canvas.findByText(/^Synced with Notion/u);
  await expect(status).toBeVisible();
  await expect(clickable(canvas.getByRole("link", { name: "Open in Notion" }))).toBe(true);
  await expect(clickable(canvas.getByRole("button", { name: "Stop syncing" }))).toBe(true);
  const row = status.closest("div");
  if (row !== null) {
    await expect(noOverlap(row)).toBe(true);
  }
}

/** The table's toolbar fits its row, so everything in it can be clicked. */
async function toolbarFits(canvasElement: HTMLElement) {
  const canvas = within(canvasElement);
  await expect(clickable(await canvas.findByRole("button", { name: "Add a view" }))).toBe(true);
  await expect(clickable(canvas.getByRole("button", { name: /^Filter/u }))).toBe(true);
  await expect(clickable(canvas.getByRole("tab", { name: /All clients/u }))).toBe(true);
  const tabs = canvas.getByRole("tablist").parentElement;
  if (tabs !== null) {
    await expect(noOverlap(tabs)).toBe(true);
  }
}

// ------------------------------------------------------------------ import

async function importFits(canvasElement: HTMLElement) {
  const dialog = await body(canvasElement).findByRole("dialog", { name: "Import from Notion" });
  const clients = await within(dialog).findByLabelText("Clients");
  await waitFor(() => expect(clients).toBeVisible());
  const databases = within(dialog).getByRole("group", { name: "Databases from Notion" });
  await expect(within(databases).getByText("7 columns")).toBeVisible();
  await expect(clickable(clients)).toBe(true);
  await expect(clickable(within(dialog).getByLabelText("Projects"))).toBe(true);
  await expect(clickable(within(dialog).getByRole("button", { name: "Cancel" }))).toBe(true);
  await userEvent.click(clients);
  const submit = within(dialog).getByRole("button", { name: "Import 1 database" });
  await expect(submit).toBeEnabled();
  await expect(clickable(submit)).toBe(true);
}

/** The import lists the databases beside the pages; ticking one imports it as a table. */
export const ImportWithDatabases: Story = {
  args: { path: IMPORT, synced: { hidden: true } },
  ...DESKTOP,
  play: async ({ canvasElement }) => {
    await importFits(canvasElement);
  },
};

/** The same on a phone. */
export const ImportWithDatabasesOnAPhone: Story = {
  args: { path: IMPORT, synced: { hidden: true } },
  ...PHONE,
  play: async ({ canvasElement }) => {
    await importFits(canvasElement);
  },
};

// ------------------------------------------------------------ a synced table

async function syncedFits(canvasElement: HTMLElement) {
  await syncRowFits(canvasElement);
  await toolbarFits(canvasElement);
  const canvas = within(canvasElement);
  for (const name of [
    "Read-only: a formula, computed in Notion",
    "Read-only: people stay Notion’s",
    "Campbooks only: never sent to Notion",
  ]) {
    const mark = canvas.getByRole("img", { name });
    mark.scrollIntoView({ block: "nearest", inline: "nearest" });
    await expect(mark).toBeVisible();
  }
  // On a phone the grid scrolls inside its box, so a far column is reached by scrolling it: it
  // only has to be there.
  const score = canvas.getByRole("button", { name: /^Score/u });
  await expect(window.innerWidth >= 768 ? clickable(score) : score.isConnected).toBe(true);
  await expect(score).toBeVisible();
  await expect(clickable(canvas.getByRole("button", { name: "Add a column" }))).toBe(true);
  // A formula's cells cannot be edited.
  await expect(within(canvas.getByRole("grid")).getAllByText("82")[0]).toBeVisible();
}

/** The clients table kept in step with Notion: the badge, locked columns, the Campbooks-only column. */
export const SyncedTable: Story = {
  args: { path: TABLE },
  ...DESKTOP,
  play: async ({ canvasElement }) => {
    await syncedFits(canvasElement);
  },
};

/** The same on a phone: the grid scrolls inside its box, the page does not. */
export const SyncedTableOnAPhone: Story = {
  args: { path: TABLE },
  ...PHONE,
  play: async ({ canvasElement }) => {
    await syncedFits(canvasElement);
  },
};

// ------------------------------------------------------------ a changed cell

async function cellConflictInTheGrid(canvasElement: HTMLElement) {
  const canvas = within(canvasElement);
  await syncRowFits(canvasElement);
  await expect(clickable(await canvas.findByRole("button", { name: "1 change to look at" }))).toBe(
    true,
  );
  const flag = await canvas.findByRole("button", {
    name: "Stage changed in Notion too: choose a version",
  });
  await expect(clickable(flag)).toBe(true);
  await userEvent.click(flag);
  const card = await body(canvasElement).findByRole("region", {
    name: "A change from Notion in Stage",
  });
  await expect(within(card).getByText("Kickoff")).toBeVisible();
  await expect(within(card).getByText("Proposal sent")).toBeVisible();
  const keep = within(card).getByRole("button", { name: "Keep mine for Stage" });
  const use = within(card).getByRole("button", { name: "Use Notion’s for Stage" });
  await expect(clickable(keep)).toBe(true);
  await expect(clickable(use)).toBe(true);
  await userEvent.click(use);
  const toast = await body(canvasElement).findByText(/Using Notion’s version/u);
  await expect(toast).toBeVisible();
  await expect(
    await canvas.findByText("Proposal sent", { selector: "span" }, { timeout: 4000 }),
  ).toBeVisible();
  await userEvent.click(await body(canvasElement).findByRole("button", { name: /Undo/u }));
  await expect(
    await canvas.findByRole("button", { name: "Stage changed in Notion too: choose a version" }),
  ).toBeVisible();
}

/** A cell changed here and in Notion: a flag in the grid opens both versions, with Undo. */
export const CellConflictInTheGrid: Story = {
  args: { path: TABLE, synced: { cellConflict: true } },
  ...DESKTOP,
  play: async ({ canvasElement }) => {
    await cellConflictInTheGrid(canvasElement);
  },
};

/** The same on a phone. */
export const CellConflictInTheGridOnAPhone: Story = {
  args: { path: TABLE, synced: { cellConflict: true } },
  ...PHONE,
  play: async ({ canvasElement }) => {
    await cellConflictInTheGrid(canvasElement);
  },
};

async function cellConflictInThePeek(canvasElement: HTMLElement) {
  const peek = await body(canvasElement).findByRole("dialog", { name: "Row" });
  const card = await within(peek).findByRole("region", { name: "A change from Notion in Stage" });
  await expect(card).toBeVisible();
  const keep = within(card).getByRole("button", { name: "Keep mine for Stage" });
  const use = within(card).getByRole("button", { name: "Use Notion’s for Stage" });
  await expect(clickable(keep)).toBe(true);
  await expect(clickable(use)).toBe(true);
  await userEvent.click(keep);
  await waitFor(() =>
    expect(
      within(peek).queryByRole("region", { name: "A change from Notion in Stage" }),
    ).toBeNull(),
  );
}

/** The same cell in the row beside the table. */
export const CellConflictInThePeek: Story = {
  args: { path: PEEK, synced: { cellConflict: true } },
  ...DESKTOP,
  play: async ({ canvasElement }) => {
    await cellConflictInThePeek(canvasElement);
  },
};

/** The same on a phone. */
export const CellConflictInThePeekOnAPhone: Story = {
  args: { path: PEEK, synced: { cellConflict: true } },
  ...PHONE,
  play: async ({ canvasElement }) => {
    await cellConflictInThePeek(canvasElement);
  },
};

// ---------------------------------------------------------- a changed column

async function columnConflictBanner(canvasElement: HTMLElement) {
  const canvas = within(canvasElement);
  const banner = await canvas.findByRole("region", { name: "Columns changed in both places" });
  await expect(
    within(banner).getByText(/“Deal size” was changed here and in Notion/u),
  ).toBeVisible();
  const keep = within(banner).getByRole("button", { name: "Keep “Deal size”" });
  const use = within(banner).getByRole("button", { name: "Use “Contract value”" });
  await expect(clickable(keep)).toBe(true);
  await expect(clickable(use)).toBe(true);
  await userEvent.click(use);
  await expect(await canvas.findByRole("columnheader", { name: "Contract value" })).toBeVisible();
  await userEvent.click(await body(canvasElement).findByRole("button", { name: /Undo/u }));
  await expect(await canvas.findByRole("columnheader", { name: "Deal size" })).toBeVisible();
}

/** A column renamed here and in Notion: a banner above the table with both names. */
export const ColumnConflictBanner: Story = {
  args: { path: TABLE, synced: { columnConflict: true } },
  ...DESKTOP,
  play: async ({ canvasElement }) => {
    await columnConflictBanner(canvasElement);
  },
};

/** The same on a phone. */
export const ColumnConflictBannerOnAPhone: Story = {
  args: { path: TABLE, synced: { columnConflict: true } },
  ...PHONE,
  play: async ({ canvasElement }) => {
    await columnConflictBanner(canvasElement);
  },
};

// ----------------------------------------------------------- removing a column

async function removeFromBothPlaces(canvasElement: HTMLElement) {
  const canvas = within(canvasElement);
  const score = await canvas.findByRole("button", { name: /^Score/u });
  await expect(clickable(score)).toBe(true);
  await userEvent.click(score);
  const remove = await body(canvasElement).findByRole("menuitem", {
    name: "Remove from both places…",
  });
  await expect(clickable(remove)).toBe(true);
  await userEvent.click(remove);
  const dialog = await body(canvasElement).findByRole("dialog", {
    name: "Remove “Score” from both places?",
  });
  await waitFor(() => expect(dialog).toBeVisible());
  await expect(within(dialog).getByText(/Campbooks keeps them for 30 days/u)).toBeVisible();
  await expect(clickable(within(dialog).getByRole("button", { name: "Cancel" }))).toBe(true);
  const confirm = within(dialog).getByRole("button", { name: "Remove from both" });
  await expect(clickable(confirm)).toBe(true);
  await userEvent.click(confirm);
  await expect(
    await body(canvasElement).findByText("Column Score removed from both places"),
  ).toBeVisible();
  await userEvent.click(await body(canvasElement).findByRole("button", { name: /Undo/u }));
  await expect(await canvas.findByRole("button", { name: /^Score/u })).toBeVisible();
}

/** Removing a synced column asks "Remove from both places?" and Undo brings it back in both. */
export const RemoveFromBothPlaces: Story = {
  args: { path: TABLE },
  ...DESKTOP,
  play: async ({ canvasElement }) => {
    await removeFromBothPlaces(canvasElement);
  },
};

/** The same on a phone. */
export const RemoveFromBothPlacesOnAPhone: Story = {
  args: { path: TABLE },
  ...PHONE,
  play: async ({ canvasElement }) => {
    await removeFromBothPlaces(canvasElement);
  },
};

// The same screens with nothing done yet: the recording does the ticking,
// opening and choosing (no `play`).

/** Pages, with the import open and the clients database offered. */
export const ImportWithDatabasesIdle: Story = {
  args: { path: IMPORT, synced: { hidden: true } },
  ...DESKTOP,
};

/** The same on a phone. */
export const ImportWithDatabasesPhoneIdle: Story = {
  args: { path: IMPORT, synced: { hidden: true } },
  ...PHONE,
};

/** The clients table, kept in step with Notion. */
export const SyncedTableIdle: Story = { args: { path: TABLE }, ...DESKTOP };

/** The same on a phone. */
export const SyncedTablePhoneIdle: Story = { args: { path: TABLE }, ...PHONE };

/** The table with a cell changed in both places. */
export const CellConflictIdle: Story = {
  args: { path: TABLE, synced: { cellConflict: true } },
  ...DESKTOP,
};

/** The same on a phone. */
export const CellConflictPhoneIdle: Story = {
  args: { path: TABLE, synced: { cellConflict: true } },
  ...PHONE,
};

/** The row beside the table, with the cell changed in both places. */
export const CellConflictPeekIdle: Story = {
  args: { path: PEEK, synced: { cellConflict: true } },
  ...DESKTOP,
};

/** The table with a column renamed in both places. */
export const ColumnConflictIdle: Story = {
  args: { path: TABLE, synced: { columnConflict: true } },
  ...DESKTOP,
};

/** The same on a phone. */
export const ColumnConflictPhoneIdle: Story = {
  args: { path: TABLE, synced: { columnConflict: true } },
  ...PHONE,
};

/** The table, for removing a column from both places. */
export const RemoveColumnIdle: Story = { args: { path: TABLE }, ...DESKTOP };

/** The same on a phone. */
export const RemoveColumnPhoneIdle: Story = { args: { path: TABLE }, ...PHONE };
