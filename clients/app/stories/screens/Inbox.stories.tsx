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
  SENT,
  SPAMMED,
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

/**
 * ↓ and ↑ move the cursor like J and K, and Enter opens the row under it.
 * In the open conversation the arrows are the browser's again (they scroll
 * the message): J and K step between conversations.
 */
export const ArrowKeys: Story = {
  args: { path: `${PATH}/primary`, api: inbox() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText(Q4.subject);
    const row = (id: string) => canvasElement.querySelector(`#conversation-${id}`);

    await userEvent.keyboard("{ArrowDown}{ArrowDown}{ArrowDown}{ArrowUp}");
    // Q4, the NDA, the Ledger, then back up to the NDA.
    await expect(row(PRIMARY[1]?.id ?? "")).toHaveAttribute("data-selected", "true");

    await userEvent.keyboard("{ArrowDown}{Enter}");
    await expect(await canvas.findByTitle("Message from The Weekly Ledger")).toBeVisible();

    await userEvent.keyboard("{ArrowDown}");
    await expect(row(LEDGER.id)).toHaveAttribute("aria-current", "true");
  },
};

/** ↓ past the last row loaded asks for the next page, then moves into it. */
export const ArrowKeysNextPage: Story = {
  args: {
    path: `${PATH}/primary`,
    api: inbox({
      [`GET ${W}/conversations`]: (_body, url) =>
        url.searchParams.get("cursor") === "more"
          ? { body: { conversations: [INVOICE], next_cursor: null } }
          : { body: { conversations: PRIMARY, next_cursor: "more" } },
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText(Q4.subject);
    const row = (id: string) => canvasElement.querySelector(`#conversation-${id}`);

    await userEvent.keyboard("{ArrowDown}".repeat(PRIMARY.length));
    const last = PRIMARY.at(-1)?.id ?? "";
    await expect(row(last)).toHaveAttribute("data-selected", "true");

    // Past the last row: the next page loads and the cursor stays put.
    await userEvent.keyboard("{ArrowDown}");
    await expect(await canvas.findByText(INVOICE.subject)).toBeVisible();
    await expect(row(last)).toHaveAttribute("data-selected", "true");

    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(row(INVOICE.id)).toHaveAttribute("data-selected", "true"));
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

/** A mailbox disconnected with its mail kept: history, read-only, nothing to act on. */
export const KeptHistory: Story = {
  args: {
    path: `${PATH}/primary/${Q4.id}`,
    api: inbox({ [`GET ${W}/inboxes`]: { body: { inboxes: [{ ...INBOX, read_only: true }] } } }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("Kept as history")).toBeVisible();
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

// A conversation whose subject, sender name and address have no break
// opportunity at all (no spaces or hyphens) and are longer than the reader is
// wide, as real mail sometimes does (a reference number as the subject, an
// automated sender).
const UNBROKEN_SUBJECT =
  "Invoice_INV2026100000184_paymentreference_0192f1a0c2d4e6f8a0b2c4d6e8f0a2b4c6d8e0f2a4b6c8d0e2f4a6b8";
const UNBROKEN_SENDER = {
  name: "WeeklyLedgerAccountsReceivableAutomatedBillingNotifications",
  address: "billing.notifications.and.statements@accountsreceivable.weeklyledger.example",
};
const UNBROKEN = { ...LEDGER, subject: UNBROKEN_SUBJECT, latest_from: UNBROKEN_SENDER };
const UNBROKEN_DETAIL = {
  ...LEDGER_DETAIL,
  conversation: UNBROKEN,
  messages: LEDGER_DETAIL.messages.map((message) => ({
    ...message,
    from: UNBROKEN_SENDER,
    subject: UNBROKEN_SUBJECT,
  })),
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

const unbroken: Pick<Story, "args" | "play"> = {
  args: {
    path: `${PATH}/primary/${LEDGER.id}`,
    api: inbox({
      [`GET ${W}/conversations`]: {
        body: {
          conversations: [UNBROKEN, ...PRIMARY.filter((c) => c.id !== LEDGER.id)],
          next_cursor: null,
        },
      },
      [`GET ${W}/conversations/${LEDGER.id}`]: { body: UNBROKEN_DETAIL },
      [`POST ${W}/conversations/${LEDGER.id}/read`]: { body: { ...UNBROKEN, unread_count: 0 } },
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const reader = await canvas.findByRole("main", { name: "Conversation" });
    const inReader = within(reader);
    const subject = await inReader.findByRole("heading", { level: 2, name: UNBROKEN_SUBJECT });
    await expect(subject).toBeVisible();
    // Long unbroken text wraps: the subject and the sender stay inside their boxes…
    for (const text of [subject, ...inReader.getAllByText(UNBROKEN_SENDER.name)]) {
      await expect(text.scrollWidth).toBeLessThanOrEqual(text.clientWidth);
    }
    // …so the page never scrolls sideways.
    await waitFor(() =>
      expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(globalThis.innerWidth),
    );
    // From 64rem the reader sits beside the list, never under it.
    if (globalThis.matchMedia("(width >= 64rem)").matches) {
      const list = canvas.getByRole("region", { name: "Conversations" });
      await expect(reader.getBoundingClientRect().top).toBe(list.getBoundingClientRect().top);
    }
  },
};

/** Long unbroken subject and sender on a phone: they wrap, nothing scrolls sideways. */
export const UnbrokenTextPhone: Story = { ...unbroken, ...at("phone", 375, 812) };

/** A narrow desktop window: the reader stays beside the list and the text wraps. */
export const UnbrokenTextNarrowDesktop: Story = { ...unbroken, ...at("narrowDesktop", 1100, 800) };

/** A wide desktop window: the text wraps within the reader's measure. */
export const UnbrokenTextDesktop: Story = { ...unbroken, ...at("desktop", 1440, 900) };

// A real inbox: many more conversations than fit in the window.
const MANY = Array.from({ length: 40 }, (_, index) => {
  const base = PRIMARY[index % PRIMARY.length] ?? Q4;
  return index < PRIMARY.length ? base : { ...base, id: `${base.id}-${String(index)}` };
});

const many: Pick<Story, "args" | "play"> = {
  args: {
    path: `${PATH}/primary/${Q4.id}`,
    api: inbox({
      [`GET ${W}/conversations`]: { body: { conversations: MANY, next_cursor: null } },
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const reader = await canvas.findByRole("main", { name: "Conversation" });
    const list = canvas.getByRole("region", { name: "Conversations" });
    const tabs = within(list).getByRole("heading", { level: 1 });
    // Forty rows are taller than the window, so the list scrolls inside its pane…
    await waitFor(() => expect(list.scrollHeight).toBeGreaterThan(list.clientHeight));
    list.scrollTop = list.scrollHeight;
    await waitFor(() => expect(list.scrollTop).toBeGreaterThan(0));
    // …while the page itself stays the window's height…
    await expect(document.documentElement.scrollHeight).toBeLessThanOrEqual(globalThis.innerHeight);
    // …the list's header stays at the top of its pane, and the reader stays in view.
    await expect(tabs.getBoundingClientRect().top).toBeGreaterThanOrEqual(
      list.getBoundingClientRect().top,
    );
    await expect(reader.getBoundingClientRect().top).toBe(0);
    await expect(reader.getBoundingClientRect().bottom).toBe(globalThis.innerHeight);
  },
};

/** A full inbox in a narrow desktop window: the list scrolls, the page doesn't. */
export const ManyConversationsNarrowDesktop: Story = { ...many, ...at("narrowDesktop", 1100, 800) };

/** A full inbox in a wide desktop window: the list scrolls, the page doesn't. */
export const ManyConversationsDesktop: Story = { ...many, ...at("desktop", 1440, 900) };

/** Sent (G then 5): by Sam's latest message, to whom, and whether they replied since. */
const sent: Story = {
  args: {
    path: `${PATH}/sent`,
    api: inbox({
      [`GET ${W}/conversations`]: { body: { conversations: SENT, next_cursor: null } },
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("To Marta Ferreira and 2 others")).toBeVisible();
    await expect(canvas.getByText("Replied")).toBeVisible();
    // The views sit after the streams, behind a divider; the page never scrolls sideways.
    await waitFor(() =>
      expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(globalThis.innerWidth),
    );
  },
};
export const SentView: Story = sent;

/** Sent on a phone: the stream chips, then the views, in a row that scrolls on its own. */
export const SentViewPhone: Story = { ...sent, ...at("phone", 375, 812) };

/** Spam with nothing in it: one plain line. */
export const EmptySpam: Story = {
  args: {
    path: `${PATH}/spam`,
    api: inbox({ [`GET ${W}/conversations`]: { body: { conversations: [], next_cursor: null } } }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByRole("heading", { name: "Nothing in Spam." })).toBeVisible();
  },
};

/** A conversation in Spam: the banner and Not spam (!); no reply, no Scout, images never load. */
export const SpamReader: Story = {
  args: {
    path: `${PATH}/spam/${LEDGER.id}`,
    api: inbox({
      [`GET ${W}/conversations`]: { body: { conversations: [SPAMMED], next_cursor: null } },
      [`GET ${W}/conversations/${LEDGER.id}`]: {
        body: { ...LEDGER_DETAIL, conversation: { ...SPAMMED, id: LEDGER.id, unread_count: 0 } },
      },
      [`GET ${W}/conversations/${LEDGER.id}/messages/${ledgerMessage}/body`]: {
        body: {
          ...ledgerBody(false),
          images_offer: false,
          images_domain: null,
          images_held: "spam",
        },
      },
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("This conversation is in Spam.")).toBeVisible();
    await expect(await canvas.findByText("Images never load in Spam.")).toBeVisible();
    await expect(canvas.queryByRole("button", { name: /^Show images/u })).toBeNull();
    await expect(canvas.getByRole("button", { name: /^Not spam/u })).toBeVisible();
  },
};

/** A conversation in Trash: the banner and Restore (#). */
export const TrashReader: Story = {
  args: {
    path: `${PATH}/trash/${Q4.id}`,
    api: inbox({
      [`GET ${W}/conversations`]: {
        body: { conversations: [{ ...Q4, trashed: true }], next_cursor: null },
      },
      [`GET ${W}/conversations/${Q4.id}`]: {
        body: { ...Q4_DETAIL, conversation: { ...Q4, trashed: true, unread_count: 0 } },
      },
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("This conversation is in Trash.")).toBeVisible();
    await expect(canvas.getByRole("button", { name: /^Restore/u })).toBeVisible();
  },
};
