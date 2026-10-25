import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, waitFor, within } from "storybook/test";

import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import { COUNTS, MARTA } from "../../test/demo/attention";
import { INBOX, STREAM_COUNTS, W } from "../../test/demo/conversations";
import {
  BUDGET,
  HANDBOOK,
  HOLIDAYS,
  IDEAS,
  OLD_NOTES,
  ONBOARDING,
  PAGES,
  viewOf,
} from "../../test/demo/pages";
import { TEAM } from "../../test/demo/tasks";
import { DUARTE, SESSION } from "../../test/demo/workspaces";

import { mockFetch, saveThemeFromGlobals, type MockRoute } from "./mock-fetch";

// The Pages place against fictional demo data: the tree, a page open beside
// it, a new page, and the trash.

const PATH = `/w/${DUARTE.id}/pages`;
const P = `${W}/pages`;

/** The pages API over a list the calls change, as the server would. */
function pages(overrides: Record<string, MockRoute> = {}) {
  let live = [...PAGES];
  let trashed = [OLD_NOTES];
  const byId = (id: string) => live.find((page) => page.id === id);
  const routes: Record<string, MockRoute> = {
    "GET /bff/session": { body: SESSION },
    "GET /bff/api/v1/workspaces": { body: { workspaces: [DUARTE] } },
    [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
    [`GET ${W}/streams`]: { body: { streams: STREAM_COUNTS } },
    [`GET ${W}/members`]: { body: { members: TEAM } },
    [`GET ${W}/conversations`]: { body: { conversations: [MARTA], next_cursor: null } },
    [`GET ${W}/tasks`]: { body: { tasks: [] } },
    [`GET ${W}/attention`]: { body: { items: [], counts: COUNTS } },
    [`GET ${P}`]: (_body, url) => ({
      body: { pages: url.searchParams.get("trashed") === "true" ? trashed : live },
    }),
    [`GET ${P}/backlinks`]: { body: { pages: [] } },
    [`POST ${P}`]: (body) => {
      const made = {
        ...IDEAS,
        id: "0192f1a0-0000-7000-8000-0000000009ee",
        title: "",
        icon: null,
        ...(body as object),
      };
      live = [...live, made];
      return { status: 201, body: viewOf(made, []) };
    },
    ...Object.fromEntries(
      [HANDBOOK, ONBOARDING, HOLIDAYS, IDEAS, BUDGET].flatMap((page) => [
        [
          `GET ${P}/${page.id}`,
          () => ({
            body: viewOf(page, [
              page === HANDBOOK
                ? "Welcome to Acme Studio. This is how we work together."
                : `Notes for ${page.title}.`,
            ]),
          }),
        ],
        [
          `POST ${P}/${page.id}/sync`,
          { body: { revision: 1, state_vector: "AA==", update: "AAA=" } },
        ],
        [
          `POST ${P}/${page.id}/trash`,
          () => {
            const gone = byId(page.id);
            live = live.filter((other) => other.id !== page.id);
            trashed = gone === undefined ? trashed : [...trashed, gone];
            return { body: { ...viewOf(page, []), trashed_at: "2026-10-13T09:00:00Z" } };
          },
        ],
        [
          `POST ${P}/${page.id}/restore`,
          () => {
            live = [...live, page];
            trashed = trashed.filter((other) => other.id !== page.id);
            return { body: viewOf(page, []) };
          },
        ],
      ]),
    ),
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

/** New page (or N P) makes a private page and opens it. */
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
