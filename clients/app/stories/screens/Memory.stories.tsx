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
// the section cards (teach, rules, answers), tested with axe in light and dark.

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
