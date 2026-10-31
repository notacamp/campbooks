import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, fn, userEvent, within } from "storybook/test";
import * as Y from "yjs";

import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import { FilterBuilder } from "../../src/modules/pages/tables/components/Builders";
import { TypeMenu } from "../../src/modules/pages/tables/components/ColumnMenus";
import type { PropertyView } from "../../src/modules/pages/tables/types";
import { COUNTS, MARTA, MARTA_DETAIL } from "../../test/demo/attention";
import { INBOX, STREAM_COUNTS, W } from "../../test/demo/conversations";
import { IDEAS, PAGES, viewOfDoc } from "../../test/demo/pages";
import {
  CLIENTS_ID,
  P,
  ROW,
  TablesServer,
  VIEW_BOARD,
  VIEW_TABLE,
  type Handler,
} from "../../test/demo/tables";
import { LEASE, TEAM } from "../../test/demo/tasks";
import { DUARTE, SESSION } from "../../test/demo/workspaces";

import { must } from "../../test/must";
import { mockFetch, saveThemeFromGlobals } from "./mock-fetch";

// Tables in the Pages place against fictional demo data: Duarte & Pessoa's
// clients as a table and a board, a row opened beside them, and a page with a
// table in it. The mock server keeps state (cells have versions, columns,
// views and rows change as they would) and starts over each time the app
// loads. The `*Idle` stories have no `play`, so a recording can drive them.

const PATH = `/w/${DUARTE.id}/pages`;
const server = new TablesServer();

/** The page that holds a table in its body. */
const HOST = { ...IDEAS, title: "Client review" };

function hostDoc(): Y.Doc {
  const doc = new Y.Doc();
  const intro = new Y.XmlElement("paragraph");
  intro.insert(0, [new Y.XmlText("Where each client stands this week:")]);
  const table = new Y.XmlElement("tableView");
  table.setAttribute("table", CLIENTS_ID);
  table.setAttribute("view", VIEW_TABLE);
  doc.getXmlFragment("default").push([intro, table]);
  return doc;
}

/** The workspace around the tables: members, the records a link points at, the tree. */
function api(prepare?: (server: TablesServer) => void): Record<string, Handler> {
  const shell: Record<string, Handler> = {
    // The app loading starts the server over.
    "GET /bff/session": () => {
      server.reset();
      prepare?.(server);
      return { body: SESSION };
    },
    "GET /bff/api/v1/workspaces": () => ({ body: { workspaces: [DUARTE] } }),
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
      body: {
        pages: [...PAGES.filter((page) => page.id !== IDEAS.id), HOST, ...server.pages()],
        tables: true,
      },
    }),
    [`GET ${W}/pages/backlinks`]: () => ({ body: { pages: [] } }),
    [`GET ${W}/pages/${HOST.id}`]: () => ({ body: viewOfDoc(HOST, hostDoc()) }),
    [`POST ${W}/pages/${HOST.id}/sync`]: () => ({
      body: { revision: 1, update: "AAA=", state_vector: "AA==" },
    }),
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
  /** Changes what the server holds after it starts over (long view names, more filters). */
  prepare?: (server: TablesServer) => void;
}

const meta = {
  title: "Screens/Tables",
  parameters: { layout: "fullscreen" },
  beforeEach: ({ args, globals }) => {
    saveThemeFromGlobals(globals);
    return mockFetch(api(args.prepare));
  },
  render: ({ path }) => <AppAt path={path} />,
} satisfies Meta<ScreenArgs>;

export default meta;
type Story = StoryObj<typeof meta>;

const TABLE = `${PATH}/${CLIENTS_ID}`;
const BOARD = `${TABLE}?view=${VIEW_BOARD}`;
const PEEK = `${TABLE}?row=${ROW.acme}`;
const INLINE = `${PATH}/${HOST.id}`;

/** The clients table: views as tabs, the filter as a chip, typed cells and the summaries. */
export const TheTable: Story = {
  args: { path: TABLE },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByRole("grid")).toBeVisible();
    await expect(await canvas.findByText("Acme Studio")).toBeVisible();
    // The other view is a tab, or in the More views menu when the row is tight.
    await expect(
      canvas.queryByRole("tab", { name: "By stage" }) ??
        canvas.getByRole("button", { name: "More views" }),
    ).toBeVisible();
    await expect(canvas.queryByText("Old Mill Dental")).toBeNull();
  },
};

/** The same clients as a board, a column per stage. */
export const TheBoard: Story = {
  args: { path: BOARD },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByRole("list", { name: "Kickoff" })).toBeVisible();
    await expect(canvas.getByRole("list", { name: "Active" })).toBeVisible();
  },
};

/** A row opened beside its table, with its properties. */
export const OpensARow: Story = {
  args: { path: TABLE },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "Open Acme Studio" }));
    const body = within(canvasElement.ownerDocument.body);
    await expect(await body.findByRole("dialog", { name: "Row" })).toBeVisible();
    await expect(await body.findByRole("textbox", { name: "Row title" })).toHaveValue(
      "Acme Studio",
    );
  },
};

/** A page with the clients table in its body. */
export const InAPage: Story = {
  args: { path: INLINE },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("Where each client stands this week:")).toBeVisible();
    await expect(await canvas.findByRole("grid")).toBeVisible();
  },
};

/** The type menu a new column starts from, in its three groups. */
export const ColumnTypesIdle: Story = {
  args: { path: TABLE },
  render: () => <TypeMenu label="Column type" onPick={fn()} />,
};

/** The filter builder with the table's own filter in it. */
export const FilterBuilderIdle: Story = {
  args: { path: TABLE },
  render: () => {
    const table = server.table(CLIENTS_ID);
    return (
      <QueryClientProvider client={new QueryClient()}>
        <FilterBuilder
          ctx={{ workspaceId: DUARTE.id, locale: "en", members: TEAM, open: fn() }}
          properties={(table?.properties ?? []) as PropertyView[]}
          filter={{ and: [{ property: P.signed, operator: "is", value: true }] }}
          pages={[]}
          onApply={fn()}
          onClose={fn()}
        />
      </QueryClientProvider>
    );
  },
};

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

/** Views with long names and a filter of two chips: the toolbar's hardest row. */
function longViews(held: TablesServer) {
  const table = held.table(CLIENTS_ID);
  const [first, second] = table?.views ?? [];
  if (first !== undefined && second !== undefined) {
    first.name = "All clients with a signed contract";
    second.name = "By stage, with every owner";
    first.config = {
      ...first.config,
      filter: {
        and: [
          { property: P.stage, operator: "is_not", value: "o-paused" },
          { property: P.signed, operator: "is", value: true },
        ],
      },
    };
  }
}

/** The Value column as narrow as the grid lets it get: the number and its sum no longer fit. */
function narrowValue(held: TablesServer) {
  const table = held.table(CLIENTS_ID);
  for (const view of table?.views ?? []) {
    view.config = {
      ...view.config,
      properties: (table?.properties ?? []).map((property) => ({
        property: property.id,
        visible: true,
        ...(property.id === P.value ? { width: 64 } : {}),
      })),
    };
  }
}

/** Whether a click on the element would land on it: nothing sits over its middle. */
function clickable(element: HTMLElement) {
  element.scrollIntoView({ block: "nearest", inline: "nearest" });
  const box = element.getBoundingClientRect();
  const hit = element.ownerDocument.elementFromPoint(
    box.left + box.width / 2,
    box.top + box.height / 2,
  );
  return hit !== null && element.contains(hit);
}

/** Nothing in the toolbar's row sits over anything else. */
function noOverlap(row: HTMLElement) {
  const boxes = [...row.children]
    .filter((child): child is HTMLElement => child instanceof HTMLElement)
    .map((child) => child.getBoundingClientRect())
    .filter((box) => box.width > 0);
  for (const [index, box] of boxes.entries()) {
    for (const other of boxes.slice(index + 1)) {
      if (box.left < other.right - 1 && other.left < box.right - 1) {
        return false;
      }
    }
  }
  return true;
}

/**
 * The toolbar of the table fits its row: nothing overlaps, so what is in it can be clicked. The
 * view that does not fit sits in the More views menu, one click away.
 */
async function toolbarFits(canvasElement: HTMLElement) {
  const canvas = within(canvasElement);
  await canvas.findByRole("grid");
  const add = await canvas.findByRole("button", { name: "Add a view" });
  await expect(clickable(add)).toBe(true);
  await expect(clickable(canvas.getByRole("button", { name: /^Filter/u }))).toBe(true);
  await expect(clickable(canvas.getByRole("tab", { name: /All clients/u }))).toBe(true);
  await expect(noOverlap(must(canvas.getByRole("tablist").parentElement))).toBe(true);
  const board = canvas.queryByRole("tab", { name: /By stage/u });
  if (board !== null) {
    await expect(board).toBeVisible();
    await expect(clickable(board)).toBe(true);
    return;
  }
  const more = canvas.getByRole("button", { name: "More views" });
  await expect(clickable(more)).toBe(true);
  await userEvent.click(more);
  await expect(
    await within(canvasElement.ownerDocument.body).findByRole("menuitem", { name: /By stage/u }),
  ).toBeVisible();
}

/** Long view names and two filter chips at 1280: the tabs and the "+" are clear of the chips. */
export const ToolbarFitsAt1280: Story = {
  args: { path: TABLE, prepare: longViews },
  ...at("desktop", 1280, 800),
  play: async ({ canvasElement }) => {
    await toolbarFits(canvasElement);
  },
};

/** The same at 1440. */
export const ToolbarFitsAt1440: Story = {
  args: { path: TABLE, prepare: longViews },
  ...at("wide", 1440, 900),
  play: async ({ canvasElement }) => {
    await toolbarFits(canvasElement);
  },
};

/** The same on a phone. */
export const ToolbarFitsOnAPhone: Story = {
  args: { path: TABLE, prepare: longViews },
  ...at("phone", 375, 812),
  play: async ({ canvasElement }) => {
    await toolbarFits(canvasElement);
  },
};

/**
 * A narrow number column ends in an ellipsis with the whole value in its title, and the phone's
 * footer shows one "+" in "New row" (the add-column "+" sits in the header, not over it).
 */
export const NarrowNumberAndNewRowOnAPhone: Story = {
  args: { path: TABLE, prepare: narrowValue },
  ...at("phone", 375, 812),
  parameters: { ...at("phone", 375, 812).parameters, cutOnPurpose: true },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByRole("grid");
    const cell = await canvas.findByText("€9,600.00");
    await expect(cell).toHaveAttribute("title", "€9,600.00");
    await expect(getComputedStyle(cell).textOverflow).toBe("ellipsis");
    const row = must(canvas.getByRole("button", { name: /^New row/u }).closest("[role=row]"));
    const add = canvas.getByRole("button", { name: "Add a column" });
    const head = must(add.closest("[role=row]"));
    await expect(head.getBoundingClientRect().height).toBeLessThan(
      must(row.parentElement).getBoundingClientRect().height,
    );
    await expect(row.querySelectorAll("button")[0]?.querySelectorAll("svg")).toHaveLength(1);
    await expect(clickable(add)).toBe(true);
  },
};

// The same screens with nothing done yet: the recording does the typing,
// picking, filtering and moving (no `play`).

/** The table (the recording edits cells, adds a column, filters and sorts). */
export const TableIdle: Story = { args: { path: TABLE }, ...at("desktop", 1440, 900) };

/** The board (the recording moves a card to another column). */
export const BoardIdle: Story = { args: { path: BOARD }, ...at("desktop", 1440, 900) };

/** A row beside its table (the recording ticks a box and changes the stage). */
export const RowPeekIdle: Story = { args: { path: PEEK }, ...at("desktop", 1440, 900) };

/** A page with a table in it. */
export const InlineTableIdle: Story = { args: { path: INLINE }, ...at("desktop", 1440, 900) };

/** The table on a phone: the grid scrolls inside its box, the page does not. */
export const TablePhoneIdle: Story = { args: { path: TABLE }, ...at("phone", 375, 812) };

/** The board on a phone. */
export const BoardPhoneIdle: Story = { args: { path: BOARD }, ...at("phone", 375, 812) };

/** A row full screen on a phone. */
export const RowPeekPhoneIdle: Story = { args: { path: PEEK }, ...at("phone", 375, 812) };

/** A page with a table in it, on a phone. */
export const InlineTablePhoneIdle: Story = { args: { path: INLINE }, ...at("phone", 375, 812) };

/** The narrow number column on a phone (the recording shows its ellipsis and the one "+"). */
export const NarrowNumberPhoneIdle: Story = {
  args: { path: TABLE, prepare: narrowValue },
  ...at("phone", 375, 812),
  parameters: { ...at("phone", 375, 812).parameters, cutOnPurpose: true },
};

/** The narrow number column on the desktop (the recording hovers a number for its title). */
export const NarrowNumberIdle: Story = {
  args: { path: TABLE, prepare: narrowValue },
  ...at("desktop", 1440, 900),
  parameters: { ...at("desktop", 1440, 900).parameters, cutOnPurpose: true },
};
