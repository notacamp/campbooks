import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, within } from "storybook/test";

import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import {
  answerFrames,
  chatWithProposals,
  CLEAR,
  CLEAR_ANSWER,
  MARTA_CHAT,
  MARTA_CHAT_ANSWERED,
  MARTA_RULE,
  WHY,
  WHY_ANSWER,
} from "../../test/demo/assistant";
import {
  CLEARED,
  COUNTS,
  ITEMS,
  MARTA,
  MARTA_DETAIL,
  NEEDS,
  PEDRO,
  PHISH,
  PHISH_DETAIL,
  SOFIA,
} from "../../test/demo/attention";
import {
  INBOX,
  PRIMARY,
  Q4,
  Q4_BODY,
  Q4_DETAIL,
  STREAM_COUNTS,
  W,
} from "../../test/demo/conversations";
import { DUARTE, SESSION } from "../../test/demo/workspaces";

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
