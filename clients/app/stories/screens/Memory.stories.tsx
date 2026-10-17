import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, within } from "storybook/test";

import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import { INBOX, LEDGER_RULE, W } from "../../test/demo/conversations";
import { ANA, DUARTE, SESSION, SOFIA } from "../../test/demo/workspaces";

import { mockFetch, saveThemeFromGlobals, type MockRoute } from "./mock-fetch";

// Settings → Scout's memory, in the real app against in-memory demo data:
// the section cards (teach, rules, answers), and a rule built by hand,
// tested with axe in light and dark. Fictional people only.

const PAGE = `/w/${DUARTE.id}/settings/memory`;
const PROFILE = { proactivity: "ask_first", asked_at: "2026-10-01T09:00:00Z", revision: 1 };

const signedIn = (extra: Record<string, MockRoute> = {}): Record<string, MockRoute> => ({
  "GET /bff/session": { body: SESSION },
  "GET /bff/api/v1/workspaces": { body: { workspaces: [DUARTE] } },
  [`GET ${W}/members`]: { body: { members: [ANA, SOFIA] } },
  [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
  [`GET ${W}/assistant/profile`]: { body: PROFILE },
  [`GET ${W}/rules`]: { body: { rules: [LEDGER_RULE] } },
  ...extra,
});

/** People only a search finds: the builder suggests no one. */
const SENDERS = [
  { name: "Joana Reis", address: "joana@northtax.example", received: 12, replied: 7 },
  { name: "Tax desk", address: "desk@northtax.example", received: 5, replied: 0 },
  { name: "Maya Okafor", address: "maya@okafor-studio.example", received: 41, replied: 23 },
];

/** `list_senders`: everyone whose name or address has every word of `q`. */
const senders: MockRoute = (_body, url) => {
  const words = (url.searchParams.get("q") ?? "").toLowerCase().split(/\s+/u);
  return {
    body: {
      senders: SENDERS.filter((each) =>
        words.every((word) => `${each.address} ${each.name}`.toLowerCase().includes(word)),
      ),
    },
  };
};

/** Building rules by hand: finding senders, previewing and saving. */
const byHand = (): Record<string, MockRoute> => {
  let next = 0;
  return signedIn({
    [`GET ${W}/inboxes`]: { body: { inboxes: [{ ...INBOX, shared: false }] } },
    [`GET ${W}/senders`]: senders,
    [`POST ${W}/inboxes/${INBOX.id}/rules/preview`]: {
      body: {
        matching: 12,
        would_change: 12,
        samples: [],
        skipped_suspicious: 0,
        skipped_member: 0,
        overrides: [],
      },
    },
    [`POST ${W}/inboxes/${INBOX.id}/rules`]: () => {
      next += 1;
      return {
        status: 201,
        body: { rule: { id: `rule-by-hand-${String(next)}` }, acted_without_asking: false },
      };
    },
  });
};

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
  title: "Screens/Scout's memory",
  parameters: { layout: "fullscreen" },
  beforeEach: ({ args, globals }) => {
    saveThemeFromGlobals(globals);
    return mockFetch(args.api);
  },
  render: ({ path }) => <AppAt path={path} />,
} satisfies Meta<ScreenArgs>;

export default meta;
type Story = StoryObj<typeof meta>;

/** One taught rule, and the answers to Scout's questions. */
export const WithRules: Story = {
  args: { path: PAGE, api: signedIn() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByRole("region", { name: "Your rules" })).toBeVisible();
    await expect(await canvas.findByText("When Scout is sure")).toBeVisible();
  },
};

/** Nothing taught yet, nothing answered. */
export const Empty: Story = {
  args: {
    path: PAGE,
    api: signedIn({
      [`GET ${W}/rules`]: { body: { rules: [] } },
      [`GET ${W}/assistant/profile`]: {
        body: { proactivity: "ask_first", asked_at: null, revision: 0 },
      },
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("No rules yet")).toBeVisible();
  },
};

/**
 * "Build a rule yourself", next to the privacy line: the builder without any
 * model call, finding senders by name (the `teach-by-hand` recording).
 */
export const TeachByHand: Story = {
  args: { path: PAGE, api: byHand() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      await canvas.findByRole("button", { name: "Build a rule yourself" }),
    ).toBeVisible();
  },
};
