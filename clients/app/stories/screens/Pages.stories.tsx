import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, waitFor, within } from "storybook/test";
import * as Y from "yjs";

import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import { fromBase64, toBase64 } from "../../src/modules/pages/editor/bytes";
import { COUNTS, MARTA, MARTA_DETAIL } from "../../test/demo/attention";
import { INBOX, STREAM_COUNTS, W } from "../../test/demo/conversations";
import {
  BUDGET,
  HANDBOOK,
  HOLIDAYS,
  IDEAS,
  JONAS,
  OLD_NOTES,
  ONBOARDING,
  PAGES,
  plainDoc,
  richDoc,
  viewOfDoc,
} from "../../test/demo/pages";
import { LEASE, TEAM } from "../../test/demo/tasks";
import { DUARTE, SESSION } from "../../test/demo/workspaces";

import { mockFetch, saveThemeFromGlobals, type MockRoute } from "./mock-fetch";

// The Pages place against fictional demo data: the tree, a page open beside
// it, a new page, and the trash. The mock server keeps state (documents
// merge, names, shares and the trash change as they would), and starts over
// each time the app loads. The `*Idle` stories have no `play`, so a recording
// can drive them.

const PATH = `/w/${DUARTE.id}/pages`;
const P = `${W}/pages`;

/** The page "New page" makes. */
const MADE = { ...IDEAS, id: "0192f1a0-0000-7000-8000-0000000009ee", title: "", icon: null };

const ALL_PAGES = [HANDBOOK, ONBOARDING, HOLIDAYS, IDEAS, BUDGET, OLD_NOTES, MADE];
const MEMBERS = [...TEAM, JONAS];

type Summary = (typeof PAGES)[number];
interface Grant { member: string; role: "reader" | "writer" | "manager" }

/** What the server holds, from the start. */
function fresh() {
  const docs = new Map<string, Y.Doc>();
  docs.set(HANDBOOK.id, richDoc(MARTA.id, LEASE.id));
  return {
    live: new Map<string, Summary>(PAGES.map((page) => [page.id, page])),
    trashed: new Map<string, Summary>([[OLD_NOTES.id, OLD_NOTES]]),
    docs,
    grants: new Map<string, Grant[]>(),
    revisions: new Map<string, number>(),
  };
}

/** The pages API over state the calls change, as the server would. */
function pages(overrides: Record<string, MockRoute> = {}) {
  let state = fresh();
  const find = (id: string): Summary | undefined => state.live.get(id) ?? state.trashed.get(id);
  const docOf = (id: string): Y.Doc => {
    let doc = state.docs.get(id);
    if (doc === undefined) {
      const page = find(id);
      doc = plainDoc(
        ...(page === undefined || page.id === MADE.id ? [] : [`Notes for ${page.title}.`]),
      );
      state.docs.set(id, doc);
    }
    return doc;
  };
  const bump = (id: string) => {
    const next = (state.revisions.get(id) ?? 1) + 1;
    state.revisions.set(id, next);
    return next;
  };
  const crumbs = (page: Summary) => {
    const trail: { id: string; title: string; icon: string | null }[] = [];
    let parent = find(page.parent ?? "");
    while (parent !== undefined) {
      trail.unshift({ id: parent.id, title: parent.title, icon: parent.icon ?? null });
      parent = find(parent.parent ?? "");
    }
    return trail;
  };
  const view = (page: Summary) =>
    viewOfDoc(page, docOf(page.id), {
      revision: state.revisions.get(page.id) ?? 1,
      ancestors: crumbs(page),
      grants: state.grants.get(page.id) ?? [],
    });
  const change = (id: string, edit: (page: Summary) => Summary) => {
    const page = state.live.get(id);
    if (page === undefined) {
      return { status: 404 };
    }
    const next = { ...edit(page), meta_revision: page.meta_revision + 1 };
    state.live.set(id, next);
    return { body: next };
  };
  const per: Record<string, MockRoute> = {};
  for (const page of ALL_PAGES) {
    const id = page.id;
    per[`GET ${P}/${id}`] = () => {
      const found = find(id);
      return found === undefined ? { status: 404 } : { body: view(found) };
    };
    per[`POST ${P}/${id}/sync`] = (body) => {
      const { state_vector: vector, update } = body as { state_vector: string; update?: string };
      const doc = docOf(id);
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
    per[`PATCH ${P}/${id}`] = (body) => {
      const { title, icon } = body as { title: string; icon?: string | null };
      return change(id, (current) => ({ ...current, title, icon: icon ?? null }));
    };
    per[`POST ${P}/${id}/move`] = (body) => {
      const { parent, place, position } = body as {
        parent?: string | null;
        place?: Summary["place"];
        position?: string;
      };
      return change(id, (current) => ({
        ...current,
        parent: parent ?? null,
        place: place ?? current.place,
        position: position ?? current.position,
      }));
    };
    per[`POST ${P}/${id}/trash`] = () => {
      const gone = state.live.get(id);
      if (gone === undefined) {
        return { status: 409 };
      }
      state.live.delete(id);
      const stamped = { ...gone, trashed_at: "2026-10-13T09:00:00Z" };
      state.trashed.set(id, stamped);
      return { body: stamped };
    };
    per[`POST ${P}/${id}/restore`] = () => {
      const back = state.trashed.get(id);
      if (back === undefined) {
        return { status: 409 };
      }
      state.trashed.delete(id);
      const open = { ...back, trashed_at: null };
      state.live.set(id, open);
      return { body: open };
    };
    per[`DELETE ${P}/${id}`] = () => {
      state.trashed.delete(id);
      return { status: 204 };
    };
    for (const member of MEMBERS) {
      per[`PUT ${P}/${id}/grants/${member.user_id}`] = (body) => {
        const { role } = body as { role: Grant["role"] };
        const others = (state.grants.get(id) ?? []).filter((g) => g.member !== member.user_id);
        state.grants.set(id, [...others, { member: member.user_id, role }]);
        bump(id);
        return { body: { member: member.user_id, role } };
      };
      per[`DELETE ${P}/${id}/grants/${member.user_id}`] = () => {
        state.grants.set(
          id,
          (state.grants.get(id) ?? []).filter((g) => g.member !== member.user_id),
        );
        bump(id);
        return { status: 204 };
      };
    }
  }
  const routes: Record<string, MockRoute> = {
    // The app loading starts the server over.
    "GET /bff/session": () => {
      state = fresh();
      return { body: SESSION };
    },
    "GET /bff/api/v1/workspaces": { body: { workspaces: [DUARTE] } },
    [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
    [`GET ${W}/streams`]: { body: { streams: STREAM_COUNTS } },
    [`GET ${W}/members`]: { body: { members: MEMBERS } },
    [`GET ${W}/conversations`]: { body: { conversations: [MARTA], next_cursor: null } },
    [`GET ${W}/conversations/${MARTA.id}`]: { body: MARTA_DETAIL },
    [`GET ${W}/tasks`]: { body: { tasks: [] } },
    [`GET ${W}/tasks/${LEASE.id}`]: { body: { task: LEASE } },
    [`GET ${W}/correspondents`]: { body: { correspondents: [] } },
    [`GET ${W}/attention`]: { body: { items: [], counts: COUNTS } },
    [`GET ${P}`]: (_body, url) => ({
      body: {
        pages: [
          ...(url.searchParams.get("trashed") === "true" ? state.trashed : state.live).values(),
        ],
      },
    }),
    [`GET ${P}/backlinks`]: { body: { pages: [] } },
    [`POST ${P}`]: () => {
      state.live.set(MADE.id, MADE);
      return { status: 201, body: view(MADE) };
    },
    ...per,
    ...overrides,
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
  title: "Screens/Pages",
  parameters: { layout: "fullscreen" },
  beforeEach: ({ args, globals }) => {
    saveThemeFromGlobals(globals);
    return mockFetch(args.api);
  },
  render: ({ path }) => <AppAt path={path} />,
} satisfies Meta<ScreenArgs>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Workspace, Private and Shared with you; sub-pages fold under their parent. */
export const TheTree: Story = {
  args: { path: PATH, api: pages() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("Team handbook")).toBeVisible();
    await expect(canvas.getByText("Ideas for the Acme Studio kickoff")).toBeVisible();
    await expect(canvas.getByText("Budget draft")).toBeVisible();
    await expect(canvas.queryByText("Onboarding checklist")).toBeNull();
    await userEvent.click(
      await canvas.findByRole("button", { name: "Show the sub-pages of Team handbook" }),
    );
    await expect(await canvas.findByText("Onboarding checklist")).toBeVisible();
  },
};

/** A page open beside the tree, with its text in the editor. */
export const PageOpen: Story = {
  args: { path: `${PATH}/${HANDBOOK.id}`, api: pages() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      await canvas.findByText("Welcome to Acme Studio. This is how we work together."),
    ).toBeVisible();
    await expect(await canvas.findByRole("button", { name: "Share" })).toBeVisible();
  },
};

/**
 * Clicking the end of a page and typing lands the text there (not at the
 * start), and keeps landing there after the text has been saved.
 */
export const TypingKeepsTheCaret: Story = {
  args: { path: `${PATH}/${HANDBOOK.id}`, api: pages() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const editor = await canvas.findByRole("textbox", { name: "Page text" });
    const last = await within(editor).findByText("Notes from the kickoff go below.");
    await userEvent.click(last);
    await userEvent.keyboard("{End}{Enter}Pages save as you type.");
    // The save goes out and is answered...
    await canvas.findByText("Saving…");
    await canvas.findByText("Saved");
    // ...and the caret is where it was.
    await userEvent.keyboard("{Enter}Second line");
    await expect(editor.querySelector("h1")).toHaveTextContent("How we work");
    const paragraphs = [...editor.querySelectorAll(":scope > p")].map((p) => p.textContent);
    await expect(paragraphs.slice(-3)).toEqual([
      "Notes from the kickoff go below.",
      "Pages save as you type.",
      "Second line",
    ]);
  },
};

/** New page (or N G) makes a private page and opens it. */
export const NewPage: Story = {
  args: { path: `${PATH}?new=1`, api: pages() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByRole("textbox", { name: "Page title" })).toBeVisible();
  },
};

/** Moving a page to the trash offers Undo. */
export const TrashWithUndo: Story = {
  args: { path: `${PATH}/${IDEAS.id}`, api: pages() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "More" }));
    const body = within(canvasElement.ownerDocument.body);
    await userEvent.click(await body.findByRole("button", { name: "Move to trash" }));
    await expect(await body.findByText(/moved to the trash/u)).toBeVisible();
    await waitFor(() => expect(canvas.queryByText("Ideas for the Acme Studio kickoff")).toBeNull());
  },
};

/** The trash: what was deleted, with Restore. */
export const Trash: Story = {
  args: { path: PATH, api: pages() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "Trash" }));
    await expect(await canvas.findByText("Old meeting notes")).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Restore" })).toBeVisible();
  },
};

/** The first run: no pages yet. */
export const Empty: Story = {
  args: {
    path: PATH,
    api: pages({ [`GET ${P}`]: { body: { pages: [] } } }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("Write it down once")).toBeVisible();
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

// The same screens with nothing done yet: the recording does the typing,
// ticking, sharing and trashing (no `play`).

/** The tree, nothing open. */
export const TheTreeIdle: Story = {
  args: { path: PATH, api: pages() },
  ...at("desktop", 1440, 900),
};

/** A rich page open beside the tree. */
export const PageOpenIdle: Story = {
  args: { path: `${PATH}/${HANDBOOK.id}`, api: pages() },
  ...at("desktop", 1440, 900),
};

/** A new page, ready for its title. */
export const NewPageIdle: Story = {
  args: { path: `${PATH}?new=1`, api: pages() },
  ...at("desktop", 1440, 900),
};

/** The trash (the recording opens it). */
export const TrashIdle: Story = { args: { path: PATH, api: pages() }, ...at("desktop", 1440, 900) };

/** The tree screen on a phone. */
export const TheTreePhoneIdle: Story = {
  args: { path: PATH, api: pages() },
  ...at("phone", 375, 812),
};

/** A page open on a phone. */
export const PageOpenPhoneIdle: Story = {
  args: { path: `${PATH}/${HANDBOOK.id}`, api: pages() },
  ...at("phone", 375, 812),
};
