import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, within } from "storybook/test";
import * as Y from "yjs";

import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import { fromBase64, toBase64 } from "../../src/modules/pages/editor/bytes";
import { COUNTS } from "../../test/demo/attention";
import { INBOX, STREAM_COUNTS } from "../../test/demo/conversations";
import {
  CONNECTION,
  GOOGLE_WAY_IN,
  NOTION_CONNECTION,
  ZOHO_WAY_IN,
} from "../../test/demo/identity";
import {
  CONFLICT_BLOCK,
  NOTION_PAGES,
  notionDoc,
  notionLink,
  plainDoc,
  viewOfDoc,
} from "../../test/demo/pages";
import { TEAM } from "../../test/demo/tasks";
import { ANA, DUARTE, HOUSEHOLD, SESSION } from "../../test/demo/workspaces";

import { mockFetch, saveThemeFromGlobals, type MockRoute } from "./mock-fetch";

// Notion in the real app (router, session gate, workspace frame) against a
// small in-memory fake Notion: Settings → Notion (connect), the import
// (pick pages, sub-pages come along), a synced page with a conflict (three
// choices and Undo), a block kept in Notion, stopping the sync, and
// `/connect/notion` (the listing's installation address). The mock keeps its
// state and starts over each time the app loads. Connect Notion leaves the
// app for "Notion": in these stories that is the connected story. The `*Idle`
// stories have no `play`, so a recording can drive them.

const W = `/bff/api/v1/w/${DUARTE.id}`;
const P = `${W}/pages`;
const SETTINGS = `/w/${DUARTE.id}/settings/mailboxes`;
const PAGES_PATH = `/w/${DUARTE.id}/pages`;
const story = (id: string) => `/iframe.html?id=screens-notion--${id}&viewMode=story`;

/** The Pages the import makes, by their Notion page. */
const pageIdOf = (notionPage: string) => {
  const at = NOTION_PAGES.findIndex((page) => page.notion_page === notionPage);
  return `0192f1a0-0000-7000-8000-0000000009${(208 + at).toString(16)}`;
};
const ROADMAP = NOTION_PAGES[0]?.notion_page ?? "";
const ROADMAP_PAGE = pageIdOf(ROADMAP);

type Summary = Parameters<typeof viewOfDoc>[0];

const summary = (notionPage: string, title: string, position: string): Summary => ({
  id: pageIdOf(notionPage),
  parent: null,
  place: "private",
  position,
  title,
  icon: null,
  has_children: false,
  role: "manager",
  created_by: ANA.user_id,
  edited_at: "2026-10-25T09:00:00Z",
  trashed_at: null,
  meta_revision: 1,
  kind: "page",
});

interface Choice {
  keep: "mine" | "notion" | "both";
  ours: string;
  theirs: string;
}

const textOf = (element: Y.XmlElement | undefined): string =>
  element?.firstChild instanceof Y.XmlText ? String(element.firstChild) : "";

const paragraph = (text: string, bid?: string): Y.XmlElement => {
  const element = new Y.XmlElement("paragraph");
  if (bid !== undefined) {
    element.setAttribute("bid", bid);
  }
  element.insert(0, [new Y.XmlText(text)]);
  return element;
};

const conflictElement = (theirs: string): Y.XmlElement => {
  const element = new Y.XmlElement("conflict");
  element.setAttribute("of", CONFLICT_BLOCK);
  element.setAttribute("hash", "ab".repeat(32));
  element.setAttribute("edited", "2026-10-25T09:00:00.000Z");
  element.setAttribute("removed", false as unknown as string);
  element.insert(0, [paragraph(theirs)]);
  return element;
};

/** The block the conflict is about, and where the conflict sits after it. */
function locate(doc: Y.Doc) {
  const fragment = doc.getXmlFragment("default");
  const children = fragment.toArray();
  const at = children.findIndex(
    (child) => child instanceof Y.XmlElement && child.getAttribute("bid") === CONFLICT_BLOCK,
  );
  const ours = children[at];
  return { fragment, at, ours: ours instanceof Y.XmlElement ? ours : undefined };
}

/** What the server does to the page when the member chooses. */
function settle(doc: Y.Doc, keep: Choice["keep"]): Choice | undefined {
  const { fragment, at, ours } = locate(doc);
  const conflict = fragment.toArray()[at + 1];
  if (
    ours === undefined ||
    !(conflict instanceof Y.XmlElement) ||
    conflict.nodeName !== "conflict"
  ) {
    return undefined;
  }
  const record: Choice = {
    keep,
    ours: textOf(ours),
    theirs: textOf(conflict.firstChild instanceof Y.XmlElement ? conflict.firstChild : undefined),
  };
  doc.transact(() => {
    fragment.delete(at + 1, 1);
    if (keep === "notion" && ours.firstChild instanceof Y.XmlText) {
      ours.firstChild.delete(0, ours.firstChild.length);
      ours.firstChild.insert(0, record.theirs);
    }
    if (keep === "both") {
      fragment.insert(at + 1, [paragraph(record.theirs, "0192f1a0-0000-7000-8000-0000000008c1")]);
    }
  });
  return record;
}

/** Take the choice back: the conflict returns, and the block as it was. */
function unsettle(doc: Y.Doc, record: Choice) {
  const { fragment, at, ours } = locate(doc);
  if (ours === undefined) {
    return;
  }
  doc.transact(() => {
    if (record.keep === "both") {
      fragment.delete(at + 1, 1);
    }
    if (record.keep === "notion" && ours.firstChild instanceof Y.XmlText) {
      ours.firstChild.delete(0, ours.firstChild.length);
      ours.firstChild.insert(0, record.ours);
    }
    fragment.insert(at + 1, [conflictElement(record.theirs)]);
  });
}

interface Options {
  connected?: boolean;
  imported?: readonly string[];
  workspaces?: readonly (typeof DUARTE | typeof HOUSEHOLD)[];
  /** `connectors.start_notion` answers this instead of the connected story. */
  start?: MockRoute;
  importFails?: boolean;
}

/** The fake Notion's side of the app. */
function server({
  connected = false,
  imported = [],
  workspaces = [DUARTE],
  start,
  importFails = false,
}: Options = {}) {
  const fresh = () => ({
    connected,
    pages: new Map<string, Summary>(
      imported.map((id) => [pageIdOf(id), summary(id, titleOf(id), "a0")]),
    ),
    docs: new Map<string, Y.Doc>(),
    links: new Map<string, ReturnType<typeof notionLink>>(),
    revisions: new Map<string, number>(),
    resolutions: new Map<string, Choice>(),
  });
  let state = fresh();
  for (const id of imported) {
    link(id);
  }
  function titleOf(notionPage: string) {
    return NOTION_PAGES.find((page) => page.notion_page === notionPage)?.title ?? "Imported";
  }
  function link(notionPage: string) {
    const page = pageIdOf(notionPage);
    state.links.set(
      page,
      notionLink(page, notionPage, { conflicts: notionPage === ROADMAP ? 1 : 0 }),
    );
    state.docs.set(
      page,
      notionPage === ROADMAP
        ? notionDoc()
        : plainDoc(`${titleOf(notionPage)}, imported from Notion.`),
    );
  }
  const bump = (id: string) => {
    const next = (state.revisions.get(id) ?? 1) + 1;
    state.revisions.set(id, next);
    return next;
  };
  const per: Record<string, MockRoute> = {};
  for (const notionPage of NOTION_PAGES.map((page) => page.notion_page)) {
    const id = pageIdOf(notionPage);
    per[`GET ${P}/${id}`] = () => {
      const found = state.pages.get(id);
      const doc = state.docs.get(id);
      return found === undefined || doc === undefined
        ? { status: 404 }
        : {
            body: viewOfDoc(found, doc, {
              revision: state.revisions.get(id) ?? 1,
            }),
          };
    };
    per[`POST ${P}/${id}/sync`] = (body) => {
      const { state_vector: vector, update } = body as { state_vector: string; update?: string };
      const doc = state.docs.get(id) ?? new Y.Doc();
      if (update !== undefined) {
        Y.applyUpdate(doc, fromBase64(update));
        bump(id);
      }
      return {
        body: {
          revision: state.revisions.get(id) ?? 1,
          state_vector: toBase64(Y.encodeStateVector(doc)),
          update: toBase64(Y.encodeStateAsUpdate(doc, fromBase64(vector))),
        },
      };
    };
    per[`POST ${P}/${id}/notion/conflicts/${CONFLICT_BLOCK}`] = (body) => {
      const { keep } = body as { keep: Choice["keep"] };
      const doc = state.docs.get(id);
      const record = doc === undefined ? undefined : settle(doc, keep);
      if (record === undefined) {
        return { status: 409, body: { status: 409, code: "no_conflict", title: "No conflict" } };
      }
      const resolution = `0192f1a0-0000-7000-8000-0000000008d${String(state.resolutions.size)}`;
      state.resolutions.set(resolution, record);
      const current = state.links.get(id);
      if (current !== undefined) {
        state.links.set(id, { ...current, conflicts: 0, last_outbound_at: "2026-10-25T09:05:00Z" });
      }
      return { body: { resolution, revision: bump(id), conflicts: 0 } };
    };
    per[`POST ${P}/${id}/notion/stop`] = () => {
      const current = state.links.get(id);
      if (current === undefined) {
        return { status: 409, body: { status: 409, code: "not_syncing", title: "Not syncing" } };
      }
      const stopped = {
        ...current,
        state: "stopped" as const,
        stopped_reason: "stopped_by_member" as const,
      };
      state.links.set(id, stopped);
      return { body: stopped };
    };
    for (const resolution of ["0", "1", "2", "3"].map(
      (n) => `0192f1a0-0000-7000-8000-0000000008d${n}`,
    )) {
      per[`POST ${P}/${id}/notion/resolutions/${resolution}/undo`] = () => {
        const record = state.resolutions.get(resolution);
        const doc = state.docs.get(id);
        if (record === undefined || doc === undefined) {
          return { status: 409, body: { status: 409, code: "cannot_undo", title: "Cannot undo" } };
        }
        unsettle(doc, record);
        state.resolutions.delete(resolution);
        const current = state.links.get(id);
        if (current !== undefined) {
          state.links.set(id, { ...current, conflicts: 1 });
        }
        return { body: { revision: bump(id), conflicts: 1 } };
      };
    }
  }
  const mine = () => state.connected;
  const routes: Record<string, MockRoute> = {
    // The app loading starts the fake Notion over.
    "GET /bff/session": () => {
      state = fresh();
      for (const id of imported) {
        link(id);
      }
      return { body: SESSION };
    },
    "GET /bff/api/v1/workspaces": { body: { workspaces } },
    "GET /bff/api/v1/identity/held-grants": { body: { items: [] } },
    "GET /bff/api/v1/identity/passkeys": { body: { items: [] } },
    "GET /bff/api/v1/identity/ways-in": {
      body: {
        items: [GOOGLE_WAY_IN, ZOHO_WAY_IN],
        providers: ["google", "zoho"],
        mailbox_providers: ["google", "zoho"],
      },
    },
    [`GET ${W}/connections`]: () => ({
      body: { connections: mine() ? [CONNECTION, NOTION_CONNECTION] : [CONNECTION] },
    }),
    [`DELETE ${W}/connections/${NOTION_CONNECTION.id}`]: () => {
      state.connected = false;
      return { status: 204 };
    },
    [`POST ${W}/connections/notion/authorizations`]: start ?? {
      status: 201,
      body: { authorize_url: story("connected-idle"), expires_at: "2026-10-25T09:10:00Z" },
    },
    [`GET ${W}/connections/${NOTION_CONNECTION.id}/notion/pages`]: () => ({
      body: {
        pages: NOTION_PAGES.map((page) => ({
          ...page,
          linked: page.linked || state.pages.has(pageIdOf(page.notion_page)),
        })),
        truncated: false,
        requests: 1,
      },
    }),
    [`POST ${P}/notion/imports`]: (body) => {
      if (importFails) {
        return { status: 500, body: { status: 500, title: "Problem" } };
      }
      const asked = (body as { pages: { notion_page: string }[] }).pages;
      const made: { page: string; notion_page: string }[] = [];
      const wanted = [
        ...asked.map((page) => page.notion_page),
        // Sub-pages come along when the first read finds them.
        ...NOTION_PAGES.filter((page) => asked.some((a) => a.notion_page === page.parent)).map(
          (page) => page.notion_page,
        ),
      ];
      for (const notionPage of wanted) {
        const id = pageIdOf(notionPage);
        if (!state.pages.has(id)) {
          const sub = NOTION_PAGES.find((page) => page.notion_page === notionPage)?.parent;
          state.pages.set(id, {
            ...summary(notionPage, titleOf(notionPage), `a${String(state.pages.size)}`),
            ...(sub === null || sub === undefined ? {} : { parent: pageIdOf(sub) }),
          });
          link(notionPage);
          if (asked.some((page) => page.notion_page === notionPage)) {
            made.push({ page: id, notion_page: notionPage });
          }
        }
      }
      return { status: 201, body: { pages: made, skipped: 0 } };
    },
    [`GET ${P}/notion/links`]: () => ({ body: { links: [...state.links.values()] } }),
    [`GET ${P}`]: () => ({ body: { pages: [...state.pages.values()] } }),
    [`GET ${P}/backlinks`]: { body: { pages: [] } },
    [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
    [`GET ${W}/streams`]: { body: { streams: STREAM_COUNTS } },
    [`GET ${W}/members`]: { body: { members: TEAM } },
    [`GET ${W}/conversations`]: { body: { conversations: [], next_cursor: null } },
    [`GET ${W}/tasks`]: { body: { tasks: [] } },
    [`GET ${W}/correspondents`]: { body: { correspondents: [] } },
    [`GET ${W}/attention`]: { body: { items: [], counts: COUNTS } },
    ...per,
  };
  return routes;
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
  api: Record<string, MockRoute>;
}

const meta = {
  title: "Screens/Notion",
  parameters: { layout: "fullscreen" },
  beforeEach: ({ args, globals }) => {
    saveThemeFromGlobals(globals);
    return mockFetch(args.api);
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

const NOTION_PATH = (extra = "") => `${PAGES_PATH}${extra}`;
const ROADMAP_PATH = `${PAGES_PATH}/${ROADMAP_PAGE}`;

/** Settings → Notion before connecting: what it shares, and Connect Notion. */
export const NotConnected: Story = {
  args: { path: SETTINGS, api: server() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByRole("button", { name: "Connect Notion" })).toBeVisible();
    await expect(canvas.getByText(/Only the pages you pick in Notion’s own screen/u)).toBeVisible();
  },
};

/** Connected: the connection, with Import pages. */
export const Connected: Story = {
  args: { path: SETTINGS, api: server({ connected: true }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByRole("link", { name: "Import pages from Acme" })).toBeVisible();
    await expect(canvas.queryByRole("button", { name: "Connect Notion" })).toBeNull();
  },
};

/** Notion needs the member to connect again. */
export const NeedsReconnect: Story = {
  args: {
    path: SETTINGS,
    api: {
      ...server({ connected: true }),
      [`GET ${W}/connections`]: {
        body: { connections: [CONNECTION, { ...NOTION_CONNECTION, health: "needs_reauth" }] },
      },
    },
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByRole("button", { name: "Reconnect Acme" })).toBeVisible();
  },
};

/** An instance without Notion says so when Connect Notion is pressed. */
export const NotAvailable: Story = {
  args: {
    path: SETTINGS,
    api: server({
      start: {
        status: 409,
        body: { status: 409, code: "provider_not_configured", title: "No Notion" },
      },
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "Connect Notion" }));
    await expect(await canvas.findByText("Notion isn’t available on this instance.")).toBeVisible();
  },
};

/** The import: the shared pages, with their sub-pages. */
export const ImportDialog: Story = {
  args: { path: NOTION_PATH("?import=notion"), api: server({ connected: true }) },
  play: async ({ canvasElement }) => {
    const body = within(canvasElement.ownerDocument.body);
    const dialog = await body.findByRole("dialog", { name: "Import from Notion" });
    await expect(await within(dialog).findByLabelText("Product roadmap")).toBeVisible();
    await expect(within(dialog).getByText("2 sub-pages")).toBeVisible();
    await userEvent.click(within(dialog).getByLabelText("Product roadmap"));
    await expect(within(dialog).getByRole("button", { name: "Import 3 pages" })).toBeEnabled();
  },
};

/** A synced page: the badge, a conflict with both versions, a block kept in Notion. */
export const SyncedPage: Story = {
  args: { path: ROADMAP_PATH, api: server({ connected: true, imported: [ROADMAP] }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText(/^Synced with Notion/u)).toBeVisible();
    await expect(await canvas.findByText("Kept in Notion · Columns")).toBeVisible();
    await expect(await canvas.findByRole("button", { name: "Keep both" })).toBeVisible();
    await expect(canvas.getByRole("button", { name: "1 change to look at" })).toBeVisible();
  },
};

/** Keep both, then Undo puts the conflict back. */
export const KeepBothThenUndo: Story = {
  args: { path: ROADMAP_PATH, api: server({ connected: true, imported: [ROADMAP] }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "Keep both" }));
    const body = within(canvasElement.ownerDocument.body);
    await expect(await body.findByText(/Kept both, one under the other/u)).toBeVisible();
    await userEvent.click(await body.findByRole("button", { name: /Undo/u }));
    await expect(await canvas.findByRole("button", { name: "Keep both" })).toBeVisible();
  },
};

/** `/connect/notion` with several workspaces asks which one. */
export const ConnectNotionPicker: Story = {
  args: { path: "/connect/notion", api: server({ workspaces: [DUARTE, HOUSEHOLD] }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      await canvas.findByText("Which workspace should Notion connect to?"),
    ).toBeVisible();
  },
};

/** `/connect/notion` on an instance without Notion. */
export const ConnectNotionUnavailable: Story = {
  args: {
    path: "/connect/notion",
    api: server({
      start: {
        status: 409,
        body: { status: 409, code: "provider_not_configured", title: "No Notion" },
      },
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("Notion isn’t available on this instance")).toBeVisible();
  },
};

// The same screens with nothing done yet: the recording does the connecting,
// picking, choosing and stopping (no `play`).

/** Settings → Notion, not connected (Connect Notion opens the connected story). */
export const NotConnectedIdle: Story = {
  args: { path: SETTINGS, api: server() },
  ...at("desktop", 1440, 900),
};

/** Settings → Notion, connected. */
export const ConnectedIdle: Story = {
  args: { path: SETTINGS, api: server({ connected: true }) },
  ...at("desktop", 1440, 900),
};

/** Pages, with the import from Notion open. */
export const ImportIdle: Story = {
  args: { path: NOTION_PATH("?import=notion"), api: server({ connected: true }) },
  ...at("desktop", 1440, 900),
};

/** A synced page with a conflict. */
export const SyncedPageIdle: Story = {
  args: { path: ROADMAP_PATH, api: server({ connected: true, imported: [ROADMAP] }) },
  ...at("desktop", 1440, 900),
};

/** The same, on a phone. */
export const NotConnectedPhoneIdle: Story = {
  args: { path: SETTINGS, api: server() },
  ...at("phone", 375, 812),
};

/** The import, on a phone. */
export const ImportPhoneIdle: Story = {
  args: { path: NOTION_PATH("?import=notion"), api: server({ connected: true }) },
  ...at("phone", 375, 812),
};

/** A synced page with a conflict, on a phone. */
export const SyncedPagePhoneIdle: Story = {
  args: { path: ROADMAP_PATH, api: server({ connected: true, imported: [ROADMAP] }) },
  ...at("phone", 375, 812),
};

/** `/connect/notion`, choosing a workspace. */
export const ConnectNotionPickerIdle: Story = {
  args: { path: "/connect/notion", api: server({ workspaces: [DUARTE, HOUSEHOLD] }) },
  ...at("desktop", 1440, 900),
};
