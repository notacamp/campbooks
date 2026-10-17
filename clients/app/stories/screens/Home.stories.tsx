import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, waitFor, within } from "storybook/test";

import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import {
  CLEARED,
  COUNTS,
  ITEMS,
  MARTA,
  MARTA_DETAIL,
  NEEDS,
  PHISH,
  PHISH_DETAIL,
  SOFIA,
} from "../../test/demo/attention";
import { INBOX, STREAM_COUNTS, W } from "../../test/demo/conversations";
import { DUARTE, SESSION } from "../../test/demo/workspaces";

import { mockFetch, saveThemeFromGlobals, type MockRoute } from "./mock-fetch";

// Home with mail connected: what needs you, against fictional demo data.
// Every state, tested with axe in light and dark.

const PATH = `/w/${DUARTE.id}`;

function home(overrides: Record<string, MockRoute> = {}): Record<string, MockRoute> {
  return {
    "GET /bff/session": { body: SESSION },
    "GET /bff/api/v1/workspaces": { body: { workspaces: [DUARTE] } },
    [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
    [`GET ${W}/streams`]: { body: { streams: STREAM_COUNTS } },
    [`GET ${W}/attention`]: (_body, url) =>
      url.searchParams.get("state") === "cleared"
        ? { body: { items: CLEARED, counts: [] } }
        : { body: { items: ITEMS, counts: COUNTS } },
    [`GET ${W}/conversations`]: { body: { conversations: [...NEEDS, SOFIA], next_cursor: null } },
    [`GET ${W}/conversations/${MARTA.id}`]: { body: MARTA_DETAIL },
    [`GET ${W}/conversations/${PHISH.id}`]: { body: PHISH_DETAIL },
    [`POST ${W}/conversations/${PHISH.id}/need/resolve`]: { body: { ...PHISH, need: null } },
    [`POST ${W}/conversations/${PHISH.id}/need/reopen`]: { body: PHISH },
    ...overrides,
  };
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
  title: "Screens/Home",
  parameters: { layout: "fullscreen" },
  beforeEach: ({ args, globals }) => {
    saveThemeFromGlobals(globals);
    return mockFetch(args.api);
  },
  render: ({ path }) => <AppAt path={path} />,
} satisfies Meta<ScreenArgs>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Review first, then Reply, Do, Pay; due soonest first; no percentage anywhere. */
export const WhatNeedsYou: Story = {
  args: { path: PATH, api: home() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      await canvas.findByRole("heading", { level: 1, name: "6 things need you" }),
    ).toBeVisible();
    await expect(canvas.getByText("Marta Ferreira is waiting for your reply")).toBeVisible();
    await expect(canvas.getByText("Replied · Tile samples")).toBeVisible();
  },
};

/** Enter opens the warning: its tells first, then the sender's words, then how it was decided. */
export const WarningOpen: Story = {
  args: { path: PATH, api: home() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText("This may be phishing. Don’t use its links.");
    await userEvent.keyboard("{Enter}");
    await expect(
      await canvas.findByText(
        "The sender’s domain looks like a better-known one (at3lier-sol.example).",
      ),
    ).toBeVisible();
  },
};

/** X marks the warning safe; the toast offers Undo (Z). */
export const SafeWithUndo: Story = {
  args: { path: PATH, api: home() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText("This may be phishing. Don’t use its links.");
    await userEvent.keyboard("x");
    await waitFor(() =>
      expect(canvas.queryByText("This may be phishing. Don’t use its links.")).toBeNull(),
    );
    const body = within(canvasElement.ownerDocument.body);
    await expect(await body.findByText(/Marked safe/u)).toBeVisible();
  },
};

/** S opens Snooze: later today, tomorrow, Monday or next week, in a dialog. */
export const SnoozeDialog: Story = {
  args: { path: PATH, api: home() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText("Marta Ferreira is waiting for your reply");
    await userEvent.keyboard("j");
    await userEvent.keyboard("s");
    const body = within(canvasElement.ownerDocument.body);
    const dialog = await body.findByRole("dialog", { name: "Snooze Marta Ferreira until…" });
    // It rises in over a moment.
    await waitFor(async () => {
      await expect(dialog).toBeVisible();
    });
  },
};

/** Nothing needs you: a fact, not a promise, with Done today still there. */
export const NothingNeedsYou: Story = {
  args: {
    path: PATH,
    api: home({
      [`GET ${W}/attention`]: (_body, url) =>
        url.searchParams.get("state") === "cleared"
          ? { body: { items: CLEARED, counts: [] } }
          : { body: { items: [], counts: [] } },
    }),
  },
  play: async ({ canvasElement }) => {
    await expect(
      await within(canvasElement).findByRole("heading", { level: 1, name: "Nothing needs you" }),
    ).toBeVisible();
  },
};

/** The server could not be reached. */
export const LoadError: Story = {
  args: {
    path: PATH,
    api: home({
      [`GET ${W}/attention`]: {
        status: 503,
        body: { status: 503, code: "unavailable", title: "Unavailable" },
      },
    }),
  },
  play: async ({ canvasElement }) => {
    await expect(
      await within(canvasElement).findByText("Could not load what needs you"),
    ).toBeVisible();
  },
};

// A busy morning: forty things need you, more than fit in the window.
const MANY_NEEDS = Array.from({ length: 40 }, (_, index) => {
  const summary = NEEDS[index % NEEDS.length] ?? MARTA;
  return index < NEEDS.length ? summary : { ...summary, id: `${summary.id}-${String(index)}` };
});
const MANY_ITEMS = MANY_NEEDS.map((summary, index) => {
  const base = ITEMS[index % ITEMS.length] ?? ITEMS[0];
  return index < ITEMS.length || base === undefined
    ? base
    : {
        ...base,
        id: `${base.id}-${String(index)}`,
        record: { ...base.record, id: summary.id },
      };
}).filter((item) => item !== undefined);

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

const busy: Pick<Story, "args" | "play"> = {
  args: {
    path: PATH,
    api: home({
      [`GET ${W}/attention`]: (_body, url) =>
        url.searchParams.get("state") === "cleared"
          ? { body: { items: CLEARED, counts: [] } }
          : { body: { items: MANY_ITEMS, counts: COUNTS } },
      [`GET ${W}/conversations`]: {
        body: { conversations: [...MANY_NEEDS, SOFIA], next_cursor: null },
      },
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByRole("heading", { level: 1, name: "40 things need you" });
    const page = canvas.getByRole("main");
    const [sidebar] = canvas.getAllByRole("navigation");
    // Forty needs are taller than the window, so Home scrolls inside its pane…
    await waitFor(() => expect(page.scrollHeight).toBeGreaterThan(page.clientHeight));
    page.scrollTop = page.scrollHeight;
    await waitFor(() => expect(page.scrollTop).toBeGreaterThan(0));
    // …while the page itself stays the window's height and the sidebar stays put.
    await expect(document.documentElement.scrollHeight).toBeLessThanOrEqual(globalThis.innerHeight);
    await expect(sidebar?.getBoundingClientRect().bottom).toBe(globalThis.innerHeight);
  },
};

/** A busy Home in a narrow desktop window: Home scrolls, the page doesn't. */
export const BusyNarrowDesktop: Story = { ...busy, ...at("narrowDesktop", 1100, 800) };

/** A busy Home in a wide desktop window: Home scrolls, the page doesn't. */
export const BusyDesktop: Story = { ...busy, ...at("desktop", 1440, 900) };
