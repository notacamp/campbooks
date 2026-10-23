import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, within } from "storybook/test";

import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import {
  answered,
  answerFrames,
  chatWithProposals,
  CLEAR,
  CLEAR_ANSWER,
  MARTA_CHAT,
  MARTA_CHAT_ANSWERED,
  MARTA_RULE,
  NOTHING_CHAT,
  NOTICE_ANSWER,
  NOTICE_CHAT,
  NOTICE_WHY,
  RECENT,
  RULE_CHAT,
  WAITING_ANSWER,
  WAITING_QUESTION,
  WHY,
  WHY_ANSWER,
} from "../../test/demo/assistant";
import {
  CLEARED,
  COUNTS,
  INES,
  ITEMS,
  MARTA,
  MARTA_DETAIL,
  NEEDS,
  PEDRO,
  PHISH,
  PHISH_DETAIL,
  RUI,
  RUI_DETAIL,
  SOFIA,
  WAITING,
} from "../../test/demo/attention";
import {
  BUILDS_NOTICE,
  INBOX,
  LEDGER_RULE,
  PRIMARY,
  Q4,
  Q4_BODY,
  Q4_DETAIL,
  STREAM_COUNTS,
  W,
} from "../../test/demo/conversations";
import { ANA, DUARTE, SESSION, SOFIA as SOFIA_MEMBER } from "../../test/demo/workspaces";

import { mockFetch, saveThemeFromGlobals, type MockRoute } from "./mock-fetch";

// Scout's chat (campbooks-cloud#305, "Ask about this"): opened from a Home
// need and from the reader, against fictional demo data, with the answer
// streamed as a model writes it. The `Live` stories have no play, for
// recordings at 375px and on the desktop.

const HOME = `/w/${DUARTE.id}`;
const CHATS = `${W}/assistant/chats`;
const q4Body = Q4_DETAIL.messages[2]?.id ?? "";

/** Sam's chat about Maya's Q4 plan, nothing asked yet. */
const Q4_CHAT = {
  ...MARTA_CHAT,
  id: "0192f1a0-0000-7000-8000-000000003101",
  about: { type: "conversations.conversation", id: Q4.id },
};

/**
 * The API: Home and the inbox, and the chat, which holds the answer once it
 * was streamed (the panel asks for it again then).
 */
function api(overrides: Record<string, MockRoute> = {}): Record<string, MockRoute> {
  let asked = false;
  return {
    "GET /bff/session": { body: SESSION },
    "GET /bff/api/v1/workspaces": { body: { workspaces: [DUARTE] } },
    [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
    [`GET ${W}/streams`]: { body: { streams: STREAM_COUNTS } },
    [`GET ${W}/attention`]: (_body, url) =>
      url.searchParams.get("state") === "cleared"
        ? { body: { items: CLEARED, counts: [] } }
        : { body: { items: ITEMS, counts: COUNTS } },
    [`GET ${W}/conversations`]: (_body, url) =>
      url.searchParams.has("ids")
        ? { body: { conversations: [...NEEDS, SOFIA, PEDRO], next_cursor: null } }
        : { body: { conversations: PRIMARY, next_cursor: null } },
    [`GET ${W}/conversations/${MARTA.id}`]: { body: MARTA_DETAIL },
    [`GET ${W}/conversations/${PHISH.id}`]: { body: PHISH_DETAIL },
    [`GET ${W}/conversations/${Q4.id}`]: { body: Q4_DETAIL },
    [`GET ${W}/conversations/${Q4.id}/messages/${q4Body}/body`]: { body: Q4_BODY },
    [`POST ${W}/conversations/${Q4.id}/read`]: { body: { ...Q4, unread_count: 0 } },
    [`POST ${CHATS}`]: (body) => {
      const about = (body as { about?: { id?: string } } | undefined)?.about?.id;
      if (about === Q4.id) {
        return { status: 201, body: Q4_CHAT };
      }
      return asked ? { body: MARTA_CHAT_ANSWERED } : { status: 201, body: MARTA_CHAT };
    },
    [`POST ${CHATS}/${MARTA_CHAT.id}/questions/stream`]: () => {
      asked = true;
      return { stream: answerFrames(WHY_ANSWER), delay: 120 };
    },
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
  title: "Screens/Scout",
  parameters: { layout: "fullscreen" },
  beforeEach: ({ args, globals }) => {
    saveThemeFromGlobals(globals);
    return mockFetch(args.api);
  },
  render: ({ path }) => <AppAt path={path} />,
} satisfies Meta<ScreenArgs>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Opens Scout's chat from Marta's need on Home. */
async function openFromHome(canvasElement: HTMLElement) {
  const canvas = within(canvasElement);
  await userEvent.click(await canvas.findByText("Marta Ferreira is waiting for your reply"));
  await userEvent.click(await canvas.findByRole("button", { name: /Continue with Scout/u }));
}

/**
 * Continue with Scout on a Home need: the panel opens about Marta's email
 * with questions to tap; a tapped one is answered as it is written, with
 * what Scout looked at one click away.
 */
export const ContinueFromHome: Story = {
  args: { path: HOME, api: api() },
  play: async ({ canvasElement }) => {
    await openFromHome(canvasElement);
    const page = within(document.body);
    await userEvent.click(await page.findByRole("button", { name: WHY }));
    await expect(
      await page.findByText(/It is under Reply because Marta asks you/u, undefined, {
        timeout: 8000,
      }),
    ).toBeVisible();
    // The whole answer is written (as a model would), then stored.
    await userEvent.click(
      await page.findByText("What I looked at", undefined, { timeout: 15_000 }),
    );
    await expect(page.getByText("Read what happened to this email")).toBeVisible();
  },
};

/** The same, without a play: open it with `.` on Marta's need and tap a question (recordings). */
export const ContinueFromHomeLive: Story = {
  args: { path: HOME, api: api() },
};

/** Ask Scout in the reader (`.`): the chat about the open conversation. */
export const AskInTheReader: Story = {
  args: { path: `${HOME}/inbox/primary/${Q4.id}`, api: api() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByRole("heading", { level: 2, name: Q4.subject });
    await userEvent.keyboard(".");
    const page = within(document.body);
    await expect(await page.findByRole("button", { name: WHY })).toBeVisible();
    await expect(page.getByText(/Maya Okafor · Q4 plan/u)).toBeVisible();
  },
};

/** The reader without a play, for recordings. */
export const AskInTheReaderLive: Story = {
  args: { path: `${HOME}/inbox/primary/${Q4.id}`, api: api() },
};

/** Where Scout can't chat: still offered, and the panel says why. */
export const Unavailable: Story = {
  args: {
    path: HOME,
    api: api({
      [`POST ${CHATS}`]: {
        status: 409,
        body: { status: 409, code: "chat_unavailable", title: "Unavailable" },
      },
    }),
  },
  play: async ({ canvasElement }) => {
    await openFromHome(canvasElement);
    await expect(
      await within(document.body).findByText("Scout can’t chat on this server."),
    ).toBeVisible();
  },
};

/**
 * The API for Scout's proposals (campbooks-cloud#305, phase 2): asking
 * {@link CLEAR} answers with an archive card and a rule card; Confirm
 * archives Marta's email by Sam's own request, Undo brings it back, and the
 * rule is checked and saved in Teach Scout. Each card's state is kept as
 * the client settles it, so the chat read again shows it.
 */
function proposalsApi(): Record<string, MockRoute> {
  let asked = false;
  let archived = false;
  const states = new Map<string, string>();
  const proposals = () =>
    CLEAR_ANSWER.proposals.map((p) => ({ ...p, state: states.get(p.id) ?? p.state }) as typeof p);
  const inbox = INBOX.id;
  return api({
    [`POST ${CHATS}`]: () => ({
      body: asked ? chatWithProposals(proposals()) : MARTA_CHAT,
      status: asked ? 200 : 201,
    }),
    [`POST ${CHATS}/${MARTA_CHAT.id}/questions/stream`]: () => {
      asked = true;
      return { stream: answerFrames(CLEAR_ANSWER, "Sam wants it gone."), delay: 90 };
    },
    ...Object.fromEntries(
      CLEAR_ANSWER.proposals.map((p) => [
        `POST ${CHATS}/${MARTA_CHAT.id}/proposals/${p.id}`,
        (body: unknown) => {
          const state = (body as { state: string }).state;
          states.set(p.id, state);
          return { body: { ...p, state } };
        },
      ]),
    ),
    [`GET ${W}/conversations/${MARTA.id}`]: () => ({
      body: { ...MARTA_DETAIL, conversation: { ...MARTA_DETAIL.conversation, archived } },
    }),
    [`POST ${W}/conversations/${MARTA.id}/archive`]: () => {
      archived = true;
      return { body: { ...MARTA, archived: true } };
    },
    [`POST ${W}/conversations/${MARTA.id}/unarchive`]: () => {
      archived = false;
      return { body: MARTA };
    },
    [`POST ${W}/assistant/compile`]: { body: MARTA_RULE },
    [`POST ${W}/inboxes/${inbox}/rules/preview`]: {
      body: {
        matching: 3,
        would_change: 0,
        samples: [],
        skipped_suspicious: 0,
        skipped_member: 0,
        overrides: [],
      },
    },
    [`POST ${W}/inboxes/${inbox}/rules`]: {
      status: 201,
      body: { rule: { id: "0192f1a0-0000-7000-8000-000000003041" }, acted_without_asking: false },
    },
  });
}

/**
 * Scout proposes: asked to clear Marta's email, Scout answers with an
 * archive card and a rule card; Confirm archives it by Sam's own request,
 * with Undo.
 */
export const Proposals: Story = {
  args: { path: HOME, api: proposalsApi() },
  play: async ({ canvasElement }) => {
    await openFromHome(canvasElement);
    const page = within(document.body);
    const field = await page.findByRole("textbox", { name: "Ask about this" });
    await userEvent.type(field, `${CLEAR}{Enter}`);
    const card = await page.findByRole("region", { name: "Archive" }, { timeout: 15_000 });
    await expect(within(card).getByText("Marta Ferreira · Walkthrough on Thursday")).toBeVisible();
    await userEvent.click(within(card).getByRole("button", { name: "Confirm" }));
    await expect(await within(card).findByText("Done.")).toBeVisible();
    await userEvent.click(within(card).getByRole("button", { name: "Undo" }));
    await expect(await within(card).findByText("Undone.")).toBeVisible();
  },
};

/** The same, without a play: ask, Confirm, Undo, then review the rule (recordings). */
export const ProposalsLive: Story = {
  args: { path: HOME, api: proposalsApi() },
};

// Ask Scout from anywhere (campbooks-cloud#305, phase 3): ⌘K, Recent, Scout
// noticed, a rule, the Reply dialog and Waiting on others. The `Live`
// stories have no play, for recordings at 375px and on the desktop.

/** One Server-Sent Events frame, as the API sends it. */
function sse(event: string, data: unknown): string {
  return `event: ${event}\ndata: ${JSON.stringify(data)}\n\n`;
}

/**
 * The API for chats about anything: about nothing (⌘K; a new chat each
 * time, read again by its id once asked), about Scout's suggestion for CI,
 * about the Ledger rule, about Rui's contract; Recent lists Sam's chats;
 * Marta's Reply dialog and who Scout waits on are there to open Scout from.
 */
function everywhere(): Record<string, MockRoute> {
  const asked = new Set<string>();
  const ruiChat = {
    ...MARTA_CHAT,
    id: "0192f1a0-0000-7000-8000-000000003501",
    about: { type: "conversations.conversation", id: RUI.id },
  };
  return api({
    [`GET ${W}/attention`]: (_body, url) => {
      const state = url.searchParams.get("state");
      if (state === "cleared") {
        return { body: { items: CLEARED, counts: [] } };
      }
      if (state === "waiting") {
        return { body: { items: WAITING, counts: [{ reason: "waiting", count: WAITING.length }] } };
      }
      return { body: { items: ITEMS, counts: COUNTS } };
    },
    [`GET ${W}/conversations`]: (_body, url) =>
      url.searchParams.has("ids")
        ? { body: { conversations: [...NEEDS, SOFIA, PEDRO, RUI, INES], next_cursor: null } }
        : { body: { conversations: [...PRIMARY, RUI, INES], next_cursor: null } },
    [`GET ${W}/conversations/${RUI.id}`]: { body: RUI_DETAIL },
    [`GET ${W}/rule-notices`]: { body: { notices: [BUILDS_NOTICE] } },
    [`GET ${W}/rule-notices/${BUILDS_NOTICE.id}`]: { body: BUILDS_NOTICE },
    [`GET ${W}/rules`]: { body: { rules: [LEDGER_RULE] } },
    [`GET ${W}/rules/${LEDGER_RULE.id}`]: { body: LEDGER_RULE },
    [`GET ${W}/members`]: { body: { members: [ANA, SOFIA_MEMBER] } },
    [`GET ${W}/assistant/profile`]: {
      body: { proactivity: "ask_first", asked_at: "2026-10-01T09:00:00Z", revision: 1 },
    },
    [`GET ${W}/drafts`]: { body: { drafts: [] } },
    [`POST ${W}/conversations/${MARTA.id}/reply/suggestions`]: {
      body: {
        message_id: MARTA_DETAIL.messages.at(-1)?.id,
        intents: ["Yes, Thursday works", "Ask to move it to the afternoon"],
      },
    },
    [`POST ${CHATS}`]: (body) => {
      const about = (body as { about?: { type?: string; id?: string } } | undefined)?.about;
      switch (about?.id) {
        case undefined: {
          return { status: 201, body: NOTHING_CHAT };
        }
        case BUILDS_NOTICE.id: {
          return asked.has(NOTICE_CHAT.id)
            ? { body: answered(NOTICE_CHAT, NOTICE_WHY, NOTICE_ANSWER) }
            : { status: 201, body: NOTICE_CHAT };
        }
        case LEDGER_RULE.id: {
          return { status: 201, body: RULE_CHAT };
        }
        case RUI.id: {
          return { status: 201, body: ruiChat };
        }
        case Q4.id: {
          return { status: 201, body: Q4_CHAT };
        }
        default: {
          return asked.has(MARTA_CHAT.id)
            ? { body: MARTA_CHAT_ANSWERED }
            : { status: 201, body: MARTA_CHAT };
        }
      }
    },
    // ⌘K searches the mail as you type: nothing matches a question.
    [`GET ${W}/search`]: (_body, url) => ({
      body: {
        hits: [],
        indexing: false,
        next_cursor: null,
        query: { filters: [], text: url.searchParams.get("q") ?? "", warnings: [] },
      },
    }),
    [`GET ${CHATS}`]: { body: { chats: RECENT } },
    [`GET ${CHATS}/${MARTA_CHAT.id}`]: { body: MARTA_CHAT_ANSWERED },
    [`GET ${CHATS}/${RULE_CHAT.id}`]: {
      body: answered(RULE_CHAT, "What has it changed?", {
        ...WHY_ANSWER,
        id: "0192f1a0-0000-7000-8000-000000003312",
        text:
          "It has filed 40 issues of the Weekly Ledger under Newsletters, counting the ones " +
          "already in your inbox when you taught it. You moved none of them back.",
        steps: [],
      }),
    },
    [`GET ${CHATS}/${NOTHING_CHAT.id}`]: () => ({
      body: asked.has(NOTHING_CHAT.id)
        ? answered(NOTHING_CHAT, WAITING_QUESTION, WAITING_ANSWER)
        : NOTHING_CHAT,
    }),
    [`POST ${CHATS}/${NOTHING_CHAT.id}/questions/stream`]: () => {
      asked.add(NOTHING_CHAT.id);
      return {
        stream: answerFrames(WAITING_ANSWER, "Sam asks who owes him an answer."),
        delay: 120,
      };
    },
    [`POST ${CHATS}/${NOTICE_CHAT.id}/questions/stream`]: () => {
      asked.add(NOTICE_CHAT.id);
      return {
        stream: [
          sse("thinking", { delta: "CI writes most days and Sam rarely opens it." }),
          ...answerFrames(NOTICE_ANSWER).slice(1),
        ],
        delay: 120,
      };
    },
    [`POST ${CHATS}/${MARTA_CHAT.id}/questions/stream`]: () => {
      asked.add(MARTA_CHAT.id);
      return { stream: answerFrames(WHY_ANSWER), delay: 120 };
    },
  });
}

/** Opens the command menu from the sidebar's Search or ask (⌘K). */
async function openMenu(canvasElement: HTMLElement) {
  const canvas = within(canvasElement);
  await userEvent.click(await canvas.findByRole("button", { name: /Search or ask/u }));
  return within(document.body).findByRole("combobox");
}

/**
 * ⌘K, a question typed: "Ask Scout: <question>" opens a new chat about
 * nothing and asks it at once; Scout reads what needs Sam and answers.
 */
export const AskFromCommandMenu: Story = {
  args: { path: HOME, api: everywhere() },
  play: async ({ canvasElement }) => {
    const input = await openMenu(canvasElement);
    await userEvent.type(input, WAITING_QUESTION);
    const page = within(document.body);
    await userEvent.click(
      await page.findByRole("option", { name: `Ask Scout: ${WAITING_QUESTION}` }),
    );
    await expect(
      await page.findByText(/You’re waiting on two people/u, undefined, { timeout: 15_000 }),
    ).toBeVisible();
    await expect(page.getByText("your mail")).toBeVisible();
  },
};

/** The same, without a play: ⌘K, type a question, Ask Scout (recordings). */
export const AskFromCommandMenuLive: Story = {
  args: { path: HOME, api: everywhere() },
};

/** Recent, in the panel's header: Sam's chats by their first question; one reopens. */
export const Recent: Story = {
  args: { path: HOME, api: everywhere() },
  play: async ({ canvasElement }) => {
    await openMenu(canvasElement);
    await userEvent.click(within(document.body).getByRole("option", { name: "Ask Scout" }));
    const page = within(document.body);
    await userEvent.click(await page.findByRole("button", { name: "Recent" }));
    const menu = await page.findByRole("dialog", { name: "Recent" });
    await userEvent.click(await within(menu).findByText(WHY));
    await expect(
      (await page.findAllByText("Marta Ferreira · Walkthrough on Thursday"))[0],
    ).toBeVisible();
    await expect((await page.findAllByText(/It is under Reply because/u))[0]).toBeVisible();
  },
};

/** Recent without a play (recordings). */
export const RecentLive: Story = {
  args: { path: HOME, api: everywhere() },
};

/** Ask Scout on Scout noticed: the chat about the suggestion, and why Scout made it. */
export const AskFromScoutNoticed: Story = {
  args: { path: HOME, api: everywhere() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const note = await canvas.findByRole("region", { name: "Scout noticed" });
    await userEvent.click(within(note).getByRole("button", { name: /Ask Scout/u }));
    const page = within(document.body);
    await expect(
      await page.findByText("Scout’s suggestion for CI (builds@ci.example.com)"),
    ).toBeVisible();
    await userEvent.click(await page.findByRole("button", { name: NOTICE_WHY }));
    await expect(
      await page.findByText(/you opened 2 of them/u, undefined, { timeout: 15_000 }),
    ).toBeVisible();
  },
};

/** Scout noticed without a play (recordings). */
export const AskFromScoutNoticedLive: Story = {
  args: { path: HOME, api: everywhere() },
};

/** Scout's memory: a rule's More menu, Ask Scout about this rule (recordings). */
export const AskAboutARuleLive: Story = {
  args: { path: `${HOME}/settings/memory`, api: everywhere() },
};

/** Marta's Reply dialog on Home: Ask Scout beside it, the dialog left as it is (recordings). */
export const AskFromTheReplyDialogLive: Story = {
  args: { path: HOME, api: everywhere() },
};

/** Waiting on others: Ask Scout on Rui's row (recordings). */
export const AskFromWaitingLive: Story = {
  args: { path: HOME, api: everywhere() },
};
