import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, waitFor, within } from "storybook/test";

import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import {
  INBOX,
  INVOICE,
  LEDGER,
  LEDGER_DETAIL,
  ledgerBody,
  PRIMARY,
  Q4,
  Q4_BODY,
  Q4_DETAIL,
  STREAM_COUNTS,
  W,
} from "../../test/demo/conversations";
import { DUARTE, SESSION } from "../../test/demo/workspaces";

import { mockFetch, type MockRoute } from "./mock-fetch";

// The real inbox (router, shell, sidebar streams, list, reader) against the
// demo mailbox: every state of the screens, tested with axe in light and dark.

const PATH = `/w/${DUARTE.id}/inbox`;
const q4Body = Q4_DETAIL.messages[2]?.id ?? "";
const ledgerMessage = LEDGER_DETAIL.messages[0]?.id ?? "";

function inbox(overrides: Record<string, MockRoute> = {}): Record<string, MockRoute> {
  return {
    "GET /bff/session": { body: SESSION },
    "GET /bff/api/v1/workspaces": { body: { workspaces: [DUARTE] } },
    [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
    [`GET ${W}/streams`]: { body: { streams: STREAM_COUNTS } },
    [`GET ${W}/conversations`]: { body: { conversations: PRIMARY, next_cursor: null } },
    [`GET ${W}/conversations/${Q4.id}`]: { body: Q4_DETAIL },
    [`GET ${W}/conversations/${LEDGER.id}`]: { body: LEDGER_DETAIL },
    [`GET ${W}/conversations/${Q4.id}/messages/${q4Body}/body`]: { body: Q4_BODY },
    [`GET ${W}/conversations/${LEDGER.id}/messages/${ledgerMessage}/body`]: {
      body: ledgerBody(false),
    },
    [`POST ${W}/conversations/${Q4.id}/read`]: { body: { ...Q4, unread_count: 0 } },
    [`POST ${W}/conversations/${LEDGER.id}/read`]: { body: { ...LEDGER, unread_count: 0 } },
    [`POST ${W}/conversations/${Q4.id}/archive`]: { body: { ...Q4, archived: true } },
    [`POST ${W}/conversations/${Q4.id}/unarchive`]: { body: Q4 },
    [`POST ${W}/conversations/${Q4.id}/star`]: { body: { ...Q4, starred: true } },
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
  /** Paths whose answer never comes (the loading state). */
  hang?: string[];
}

/** `mockFetch`, with some paths left pending forever. */
function mockWithHangs(args: ScreenArgs): () => void {
  const restore = mockFetch(args.api);
  const mocked = globalThis.fetch;
  const hang = args.hang ?? [];
  globalThis.fetch = (input: RequestInfo | URL, init?: RequestInit) => {
    const url = new URL(input instanceof Request ? input.url : String(input), "http://localhost");
    return hang.includes(url.pathname)
      ? new Promise<Response>(() => {
          // Never answers.
        })
      : mocked(input, init);
  };
  return restore;
}

const meta = {
  title: "Screens/Inbox",
  parameters: { layout: "fullscreen" },
  beforeEach: ({ args }) => mockWithHangs(args),
  render: ({ path }) => <AppAt path={path} />,
} satisfies Meta<ScreenArgs>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A stream: rows newest first, unread and starred, a suggestion on a row. */
export const Stream: Story = {
  args: { path: `${PATH}/primary`, api: inbox() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText(Q4.subject)).toBeVisible();
    await expect(canvas.getByText("Suggests Newsletters")).toBeVisible();
  },
};

/** J moves the cursor through the list; Enter would open the row. */
export const KeyboardCursor: Story = {
  args: { path: `${PATH}/primary`, api: inbox() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText(Q4.subject);
    await userEvent.keyboard("jj");
  },
};

/** A thread: the newest message open in its sandboxed frame, older ones folded. */
export const Thread: Story = {
  args: { path: `${PATH}/primary/${Q4.id}`, api: inbox() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByTitle("Message from Maya Okafor")).toBeVisible();
    await expect(canvas.getByText("q4-plan-v3.pdf")).toBeVisible();
  },
};

/** An older message unfolded. */
export const ThreadUnfolded: Story = {
  args: { path: `${PATH}/primary/${Q4.id}`, api: inbox() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: /Sam Rivera/u }));
    await expect(await canvas.findByText(/Is the timeline fixed\?/u)).toBeVisible();
  },
};

/** A suggestion: never applied by itself, with Move to (1) and its reasons. */
export const SuggestionWithWhy: Story = {
  args: { path: `${PATH}/primary/${LEDGER.id}`, api: inbox() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByText("How Campbooks decided"));
    await expect(
      canvas.getByText(/It carries a mailing-list header \(List-Unsubscribe\)\./u),
    ).toBeVisible();
  },
};

/** Remote images held back, with Show images. */
export const ImagesBlocked: Story = {
  args: { path: `${PATH}/primary/${LEDGER.id}`, api: inbox() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText(/1 remote image hidden/u)).toBeVisible();
  },
};

/** After Show images. */
export const ImagesShown: Story = {
  args: {
    path: `${PATH}/primary/${LEDGER.id}`,
    api: inbox({
      [`GET ${W}/conversations/${LEDGER.id}/messages/${ledgerMessage}/body`]: {
        body: ledgerBody(true),
      },
    }),
  },
};

/** M opens the stream menu (1–4 pick). */
export const MoveMenu: Story = {
  args: { path: `${PATH}/primary/${Q4.id}`, api: inbox() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByTitle("Message from Maya Okafor");
    await userEvent.keyboard("m");
    await expect(await canvas.findByRole("dialog", { name: "Move to" })).toBeVisible();
  },
};

/** Done (E): archived, with Undo. */
export const DoneWithUndo: Story = {
  args: { path: `${PATH}/primary/${Q4.id}`, api: inbox() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByTitle("Message from Maya Okafor");
    await userEvent.keyboard("e");
    await waitFor(async () => {
      await expect(canvas.getByText(`Done · ${Q4.subject}`)).toBeVisible();
    });
  },
};

/** A stream with nothing in it. */
export const EmptyStream: Story = {
  args: {
    path: `${PATH}/updates`,
    api: inbox({ [`GET ${W}/conversations`]: { body: { conversations: [], next_cursor: null } } }),
  },
};

/** Unread only, with nothing unread. */
export const NothingUnread: Story = {
  args: {
    path: `${PATH}/receipts`,
    api: inbox({
      [`GET ${W}/conversations`]: { body: { conversations: [INVOICE], next_cursor: null } },
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText(INVOICE.subject);
    await userEvent.click(canvas.getByRole("button", { name: "Unread" }));
    await expect(
      await canvas.findByRole("heading", { name: "Nothing unread in Receipts" }),
    ).toBeVisible();
  },
};

/** No mailbox connected yet. */
export const NoMailbox: Story = {
  args: {
    path: `${PATH}/primary`,
    api: inbox({
      [`GET ${W}/inboxes`]: { body: { inboxes: [] } },
      [`GET ${W}/conversations`]: { body: { conversations: [], next_cursor: null } },
    }),
  },
};

/** The first page is on its way. */
export const Loading: Story = {
  args: { path: `${PATH}/primary`, api: inbox(), hang: [`${W}/conversations`] },
};

/** The conversation is on its way. */
export const ConversationLoading: Story = {
  args: { path: `${PATH}/primary/${Q4.id}`, api: inbox(), hang: [`${W}/conversations/${Q4.id}`] },
};

/** The list could not load. */
export const ListError: Story = {
  args: {
    path: `${PATH}/primary`,
    api: inbox({ [`GET ${W}/conversations`]: { status: 503, body: { status: 503 } } }),
  },
};

/** The conversation could not load (retryable). */
export const ConversationError: Story = {
  args: {
    path: `${PATH}/primary/${Q4.id}`,
    api: inbox({ [`GET ${W}/conversations/${Q4.id}`]: { status: 503, body: { status: 503 } } }),
  },
};

/** The conversation is gone, or no longer visible. */
export const ConversationGone: Story = {
  args: {
    path: `${PATH}/primary/${Q4.id}`,
    api: inbox({
      [`GET ${W}/conversations/${Q4.id}`]: {
        status: 404,
        body: { status: 404, code: "not_found", title: "Not found" },
      },
    }),
  },
};

/** Long names and subjects (Portuguese lengths) truncate cleanly. */
export const LongContent: Story = {
  args: {
    path: `${PATH}/primary`,
    api: inbox({
      [`GET ${W}/conversations`]: { body: { conversations: PRIMARY, next_cursor: "more" } },
    }),
  },
};
