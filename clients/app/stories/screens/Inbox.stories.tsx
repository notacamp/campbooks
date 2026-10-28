import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, waitFor, within } from "storybook/test";

import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import {
  applied,
  belongsIn,
  isStream,
  isView,
  type ConversationAction,
  type ConversationSummary,
  type Stream as StreamId,
} from "../../src/modules/conversations/api/queries";
import {
  INBOX,
  INVOICE,
  LEDGER,
  LEDGER_DETAIL,
  ledgerBody,
  NDA,
  PRIMARY,
  Q4,
  Q4_BODY,
  Q4_DETAIL,
  SENT,
  SPAMMED,
  STREAM_COUNTS,
  W,
} from "../../test/demo/conversations";
import type { Draft } from "../../src/modules/conversations/api/drafts";
import { DRAFT_LIST, NEW, REPLY_WRITTEN } from "../../test/demo/drafts";
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

/** The same conversation without a play, for recordings: the note offers Reply (campbooks-cloud#306). */
export const ThreadIdle: Story = {
  args: { path: `${PATH}/primary/${Q4.id}`, api: inbox() },
};

/** Your reply after the message Scout read: the note says so and offers no Reply (campbooks-cloud#355). */
const REPLY_TEXT = "Thanks Maya, v3 looks right. I will confirm the numbers by Thursday.";
const sentReply = Q4_DETAIL.messages[1];
const repliedMessage = sentReply && {
  ...sentReply,
  id: "0192f1a0-0000-7000-8000-0000000000e9",
  sent_at: "2026-10-14T09:12:00Z",
  text: { content: REPLY_TEXT, truncated: false },
  has_html: false,
};
export const ThreadReplied: Story = {
  args: {
    path: `${PATH}/primary/${Q4.id}`,
    api: inbox({
      [`GET ${W}/conversations/${Q4.id}`]: {
        body: {
          ...Q4_DETAIL,
          messages: [...Q4_DETAIL.messages, ...(repliedMessage ? [repliedMessage] : [])],
        },
      },
      [`GET ${W}/conversations/${Q4.id}/messages/${repliedMessage?.id ?? ""}/body`]: {
        body: { ...Q4_BODY, text: REPLY_TEXT, html: null },
      },
    }),
  },
};

/**
 * Scout's read of the message behind the open need failed for good: the note
 * says so in one muted line instead of staying blank (campbooks-cloud#375).
 */
const Q4_NEEDING = {
  ...Q4,
  need: {
    reason: "reply",
    trust: "sure",
    basis: "rules",
    question: "needs_reply",
    rule: "signals.question",
    due_on: null,
    overdue: false,
    lapses_at: null,
    urgent: false,
    message_id: q4Body,
    sent_at: "2026-10-13T12:00:00Z",
  },
} satisfies typeof Q4;

const readFailed = {
  args: {
    path: `${PATH}/primary/${Q4.id}`,
    api: inbox({
      // The list, the thread and its read mark all carry the open need: the
      // reader shows the need's note from the conversation it lists.
      [`GET ${W}/conversations`]: {
        body: {
          conversations: PRIMARY.map((c) => (c.id === Q4.id ? Q4_NEEDING : c)),
          next_cursor: null,
        },
      },
      [`GET ${W}/conversations/${Q4.id}`]: {
        body: { ...Q4_DETAIL, conversation: Q4_NEEDING, need_read_failed: true },
      },
      [`POST ${W}/conversations/${Q4.id}/read`]: { body: { ...Q4_NEEDING, unread_count: 0 } },
    }),
  },
} satisfies Story;

export const ThreadReadFailed: Story = {
  ...readFailed,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("Scout couldn\u2019t read this one.")).toBeVisible();
  },
};

/** The same, without a play, for recordings. */
export const ThreadReadFailedRecording: Story = readFailed;

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
    await userEvent.click(await canvas.findByRole("button", { name: /^You\s*→ Maya Okafor/u }));
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

/** Pinned (G then 9): what you pinned, archived or not, newest first. */
const pinned: Story = {
  args: {
    path: `${PATH}/starred`,
    api: inbox({
      [`GET ${W}/conversations`]: { body: { conversations: [NDA], next_cursor: null } },
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByRole("heading", { level: 1, name: "Pinned" })).toBeVisible();
    await expect(await canvas.findByText(NDA.subject)).toBeVisible();
  },
};
export const PinnedView: Story = pinned;

/** The sidebar on a desktop: the streams, then all six views in order. */
export const SidebarViews: Story = {
  ...pinned,
  ...at("desktop", 1440, 900),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByRole("heading", { level: 1, name: "Pinned" });
    const names = ["Sent", "Pinned", "Drafts", "Archive", "Spam", "Trash"];
    const links = canvas
      .getAllByRole("link")
      .map((link) => names.find((name) => link.textContent.startsWith(name)))
      .filter((name) => name !== undefined);
    await expect(links).toEqual(names);
  },
};

/** Pinned on a phone. */
export const PinnedViewPhone: Story = { ...pinned, ...at("phone", 375, 812) };

/** Drafts (G then 0): who each is to, the subject, a line of the words and when it was saved. */
const drafts: Story = {
  args: {
    path: `${PATH}/drafts`,
    api: inbox({ [`GET ${W}/drafts`]: { body: { drafts: DRAFT_LIST } } }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByRole("heading", { level: 1, name: "Drafts" })).toBeVisible();
    await expect(await canvas.findByText("Marta Ferreira +2")).toBeVisible();
    await expect(canvas.getByText("No recipients")).toBeVisible();
    await expect(canvas.getByText("(no subject)")).toBeVisible();
    await expect(
      canvas.getByRole("button", { name: "Delete draft: Figures for the board" }),
    ).toBeVisible();
    await waitFor(() =>
      expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(globalThis.innerWidth),
    );
  },
};
export const DraftsView: Story = drafts;

/** Drafts on a phone. */
export const DraftsViewPhone: Story = { ...drafts, ...at("phone", 375, 812) };

/**
 * A reply draft open in its thread, saved as you type (a draft held in
 * memory): the roomier composer in place, for recordings (campbooks-cloud#390).
 */
function replyDraftOpen(): Record<string, MockRoute> {
  let draft: Draft = {
    ...REPLY_WRITTEN,
    body: "Thanks, Maya. I will go through **the plan on Thursday** and send you my notes.",
  };
  return inbox({
    [`GET ${W}/drafts`]: () => ({ body: { drafts: [draft] } }),
    [`GET ${W}/drafts/${draft.id}`]: () => ({ body: draft }),
    [`PATCH ${W}/drafts/${draft.id}`]: (changes) => {
      draft = { ...draft, ...(changes as Partial<Draft>), version: draft.version + 1 };
      return { body: draft };
    },
  });
}
export const ReplyDraftOpen: Story = {
  args: { path: `${PATH}/drafts/${Q4.id}?draft=${REPLY_WRITTEN.id}`, api: replyDraftOpen() },
};

/** No drafts: one plain line. */
export const EmptyDrafts: Story = {
  args: {
    path: `${PATH}/drafts`,
    api: inbox({ [`GET ${W}/drafts`]: { body: { drafts: [] } } }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByRole("heading", { name: "No drafts." })).toBeVisible();
  },
};

/** Trash on a phone: its chip is the last in the row, scrolled into view. */
export const TrashChipPhone: Story = {
  args: {
    path: `${PATH}/trash`,
    api: inbox({ [`GET ${W}/conversations`]: { body: { conversations: [], next_cursor: null } } }),
  },
  ...at("phone", 375, 812),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const trash = await canvas.findByRole("button", { name: "Trash" });
    await expect(trash).toHaveAttribute("aria-pressed", "true");
    await waitFor(() => {
      const edge = trash.getBoundingClientRect().right;
      return expect(edge).toBeLessThanOrEqual(globalThis.innerWidth);
    });
  },
};

// A mailbox that keeps what is done to it: Done, Report spam, Delete and a
// move take a row out of the list, and their Undo brings it back.
const MORE: ConversationSummary[] = [
  {
    ...INVOICE,
    id: "0192f1a0-0000-7000-8000-0000000000e1",
    stream: "primary",
    subject: "Studio keys for the weekend",
    latest_from: { name: "Inês Duarte", address: "ines@atelier-norte.example" },
    snippet: "I left them with the concierge, ask for Rui.",
    last_message_at: "2026-10-07T17:20:00Z",
  },
  {
    ...INVOICE,
    id: "0192f1a0-0000-7000-8000-0000000000e2",
    stream: "primary",
    subject: "Claim your prize now!!!",
    latest_from: { name: "Prize Desk", address: "winner@prize-desk.example" },
    snippet: "You have been selected. Reply with your bank details to receive it.",
    last_message_at: "2026-10-06T08:02:00Z",
    unread_count: 1,
  },
  {
    ...INVOICE,
    id: "0192f1a0-0000-7000-8000-0000000000e3",
    stream: "primary",
    subject: "Photos from Saturday",
    latest_from: { name: "Tomás Pereira", address: "tomas@example.com" },
    snippet: "Here are the ones from the boat, the light was perfect.",
    last_message_at: "2026-10-05T19:45:00Z",
  },
];

const MOVES: Record<string, ConversationAction> = {
  archive: "archive",
  unarchive: "unarchive",
  trash: "trash",
  untrash: "untrash",
  spam: "spam",
  "not-spam": "not_spam",
};

function keptMailbox(): Record<string, MockRoute> {
  const mailbox = new Map(
    [...PRIMARY, ...MORE].map((conversation) => [conversation.id, conversation] as const),
  );
  const routes: Record<string, MockRoute> = {
    [`GET ${W}/conversations`]: (_body, url) => {
      const place = url.searchParams.get("view") ?? url.searchParams.get("stream") ?? "primary";
      return {
        body: {
          conversations:
            isStream(place) || isView(place)
              ? [...mailbox.values()].filter((conversation) => belongsIn(place, conversation))
              : [],
          next_cursor: null,
        },
      };
    },
  };
  const act = (id: string, action: ConversationAction) => () => {
    const conversation = mailbox.get(id);
    if (conversation === undefined) {
      return { status: 404 };
    }
    const next = applied(conversation, action);
    mailbox.set(id, next);
    return { body: next };
  };
  for (const id of mailbox.keys()) {
    for (const [path, action] of Object.entries(MOVES)) {
      routes[`POST ${W}/conversations/${id}/${path}`] = act(id, action);
    }
    routes[`POST ${W}/conversations/${id}/stream`] = (body) =>
      act(id, { stream: (body as { stream: StreamId }).stream })();
  }
  return inbox(routes);
}

/**
 * Rows leaving the list: Done (E), Report spam (!) and Delete (#) on the
 * cursor's row, or a move to another stream. Each row says what happened,
 * then folds away and the rows below close the gap; Undo (Z) brings it back.
 */
const rowsLeave: Pick<Story, "args" | "play"> = {
  args: { path: `${PATH}/primary`, api: keptMailbox() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText(Q4.subject);
    await userEvent.keyboard("je");
    await waitFor(async () => {
      await expect(canvasElement.querySelector(`#conversation-${Q4.id}`)).toHaveTextContent("Done");
    });
    // Then it is gone, and the next row has the cursor.
    await waitFor(
      async () => {
        await expect(canvasElement.querySelector(`#conversation-${Q4.id}`)).toBeNull();
      },
      { timeout: 3000 },
    );
  },
};

export const RowsLeave: Story = { ...rowsLeave, ...at("desktop", 1440, 900) };

/** The same mailbox with nothing pressed yet (the recording presses the keys). */
export const RowsLeaveIdle: Story = { args: rowsLeave.args, ...at("desktop", 1440, 900) };

/** On a phone, with nothing pressed yet. */
export const RowsLeavePhone: Story = { args: rowsLeave.args, ...at("phone", 375, 812) };

/** The people Sam exchanges mail with (`list_correspondents`), written to most first. */
const CORRESPONDENTS = [
  { address: "marta@ferreira-studio.example", name: "Marta Ferreira", wrote: 12, received: 9 },
  { address: "mario.lopes@lopes-arquitetos.example", name: "Mário Lopes", wrote: 4, received: 6 },
  { address: "omar@northwind-studio.example", name: "Omar Haddad", wrote: 1, received: 0 },
  { address: "marisa@harbour-books.example", name: "Marisa Costa", wrote: 0, received: 3 },
];

const fold = (text: string) =>
  text
    .normalize("NFD")
    .replaceAll(/\p{Diacritic}/gu, "")
    .toLowerCase();

/** A new message whose draft keeps what is typed, and the people to suggest. */
function composing(): Record<string, MockRoute> {
  let draft: Draft = NEW;
  return inbox({
    [`POST ${W}/drafts`]: () => ({ status: 201, body: draft }),
    [`GET ${W}/drafts/${NEW.id}`]: () => ({ body: draft }),
    [`PATCH ${W}/drafts/${NEW.id}`]: (body) => {
      draft = { ...draft, ...(body as Partial<Draft>), version: draft.version + 1 };
      return { body: draft };
    },
    [`GET ${W}/correspondents`]: (_body, url) => {
      const words = fold(url.searchParams.get("q") ?? "")
        .split(/[\s,]+/u)
        .filter((word) => word !== "");
      return {
        body: {
          correspondents: CORRESPONDENTS.filter((person) =>
            words.every((word) => fold(`${person.address} ${person.name}`).includes(word)),
          ),
        },
      };
    },
  });
}

/** A new message, its draft and the people to suggest, fresh for each story. */
const suggestArgs = () => ({ path: `/w/${DUARTE.id}/compose`, api: composing() });

/**
 * Writing a new message: To, Cc and Bcc suggest the people Sam exchanges
 * mail with as Sam types; ↓ and Enter add one, and someone already added
 * is not suggested again.
 */
const suggestPlay: Story["play"] = async ({ canvasElement }) => {
  const canvas = within(canvasElement);
  const to = await canvas.findByRole("combobox", { name: "To" });
  await userEvent.type(to, "mar");
  const list = await canvas.findByRole("listbox", { name: "Suggestions for To" });
  await expect(within(list).getAllByRole("option")).toHaveLength(4);
  await userEvent.keyboard("{ArrowDown}{Enter}");
  await expect(await canvas.findByText("Mário Lopes")).toBeVisible();
  await userEvent.click(canvas.getByRole("button", { name: "Cc Bcc" }));
  await userEvent.type(canvas.getByRole("combobox", { name: "Cc" }), "mar");
  const copies = await canvas.findByRole("listbox", { name: "Suggestions for Cc" });
  await waitFor(() => expect(within(copies).getAllByRole("option")).toHaveLength(3));
  // The list never makes the page scroll sideways.
  await expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(globalThis.innerWidth);
};

export const SuggestPeople: Story = {
  args: suggestArgs(),
  play: suggestPlay,
  ...at("desktop", 1440, 900),
};

export const SuggestPeoplePhone: Story = {
  args: suggestArgs(),
  play: suggestPlay,
  ...at("phone", 375, 812),
};

/** The same, with nothing typed yet (the recording types). */
export const SuggestPeopleIdle: Story = { args: suggestArgs(), ...at("desktop", 1440, 900) };

/** On a phone, with nothing typed yet. */
export const SuggestPeopleIdlePhone: Story = { args: suggestArgs(), ...at("phone", 375, 812) };
