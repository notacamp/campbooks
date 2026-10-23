import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ReplyOpenerProvider, type OpenReply } from "~/lib/reply-opener";

import {
  answered,
  answerFrames,
  ARCHIVE_PROPOSAL,
  NOTHING_CHAT,
  NOTICE_ANSWER,
  NOTICE_CHAT,
  NOTICE_WHY,
  RECENT,
  RULE_CHAT,
  WAITING_ANSWER,
  WAITING_QUESTION,
  chatWithProposals,
  MARTA_RULE,
  REPLY_PROPOSAL,
  RULE_PROPOSAL,
  RULE_SENTENCE,
  MARTA_CHAT,
  MARTA_CHAT_ANSWERED,
  MARTA_CHAT_FAILED,
  WHY,
  WHY_ANSWER,
} from "../../../../../test/demo/assistant";
import { MARTA, MARTA_DETAIL, PEDRO } from "../../../../../test/demo/attention";
import { BUILDS_NOTICE, LEDGER_RULE, W } from "../../../../../test/demo/conversations";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { ScoutChat, type ScoutChatProps } from "./ScoutChat";

const CHATS = `${W}/assistant/chats`;
const STREAM = `POST ${CHATS}/${MARTA_CHAT.id}/questions/stream`;

function renderChat(onClose = vi.fn(), openReply?: OpenReply, props: Partial<ScoutChatProps> = {}) {
  const root = createRootRoute({ component: Outlet });
  const chat = (
    <ScoutChat
      workspaceId={DUARTE.id}
      about={{ type: "conversations.conversation", id: MARTA.id }}
      onClose={onClose}
      modal={false}
      {...props}
    />
  );
  const page = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId",
    component: () =>
      openReply === undefined ? (
        chat
      ) : (
        <ReplyOpenerProvider open={openReply}>{chat}</ReplyOpenerProvider>
      ),
  });
  const reader = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId/inbox/$stream/$conversationId",
    component: () => <p>Reader</p>,
  });
  const router = createRouter({
    routeTree: root.addChildren([page, reader]),
    history: createMemoryHistory({ initialEntries: [`/w/${DUARTE.id}`] }),
  });
  return { router, onClose, ...renderWithProviders(<RouterProvider router={router} />) };
}

/** The API: the chat as `start` answers it, in turn; Marta's email; Pedro's in lists. */
function api(starts: ReturnType<typeof problem>[], more: Parameters<typeof mockApi>[0] = {}) {
  let started = 0;
  return mockApi({
    [`POST ${CHATS}`]: () => {
      const answer = starts[Math.min(started, starts.length - 1)] ?? { body: MARTA_CHAT };
      started += 1;
      return answer;
    },
    [`GET ${W}/conversations/${MARTA.id}`]: { body: MARTA_DETAIL },
    [`GET ${W}/conversations`]: { body: { conversations: [PEDRO], next_cursor: null } },
    ...more,
  });
}

describe("ScoutChat", () => {
  it("starts the chat about the conversation and offers questions to start from", async () => {
    const mocked = api([{ body: MARTA_CHAT }]);
    renderChat();

    const region = await screen.findByRole("region", { name: "Scout" });
    expect(
      await within(region).findByText(
        "Ask me about Marta Ferreira’s email, Walkthrough on Thursday.",
      ),
    ).toBeInTheDocument();
    expect(
      within(region).getByText("Marta Ferreira · Walkthrough on Thursday"),
    ).toBeInTheDocument();
    for (const question of [WHY, "What happened to this email?", "What else has Marta sent?"]) {
      expect(within(region).getByRole("button", { name: question })).toBeInTheDocument();
    }
    expect(screen.getByText("Scout asks before it changes anything.")).toBeInTheDocument();
    expect(mocked.calls.find((call) => call.route === `POST ${CHATS}`)?.body).toEqual({
      about: { type: "conversations.conversation", id: MARTA.id },
    });
  });

  it("asks a tapped question, streams the answer and lists what Scout looked at", async () => {
    const mocked = api([{ body: MARTA_CHAT }, { body: MARTA_CHAT_ANSWERED }], {
      [STREAM]: { stream: answerFrames(WHY_ANSWER) },
    });
    renderChat();

    fireEvent.click(await screen.findByRole("button", { name: WHY }));

    // The answer, and its announcement once it is done.
    expect(
      await screen.findAllByText(/It is under Reply because Marta asks you/u),
    ).not.toHaveLength(0);
    await waitFor(() => {
      expect(screen.queryByRole("button", { name: WHY })).not.toBeInTheDocument();
    });
    fireEvent.click(await screen.findByText("What I looked at"));
    expect(screen.getByText("Read what happened to this email")).toBeInTheDocument();
    expect(screen.getByText("Looked for conversations: 1 found")).toBeInTheDocument();
    expect(
      await screen.findByRole("button", { name: "Pedro Alves · Lease for the studio" }),
    ).toBeInTheDocument();
    const asked = mocked.calls.find((call) => call.route === STREAM);
    expect(asked?.body).toMatchObject({ text: WHY });
    // The answer is plain text: its list is a list, nothing is a link.
    const answer = screen.getByRole("article", { name: "Scout answered" });
    expect(within(answer).getByText("Scout read that it needs a reply, 90% sure.").tagName).toBe(
      "LI",
    );
    expect(within(answer).queryByRole("link")).not.toBeInTheDocument();
  });

  it("says Scout can't chat on this server, in place of the field", async () => {
    api([problem(409, "chat_unavailable")]);
    renderChat();

    expect(await screen.findByText("Scout can’t chat on this server.")).toBeInTheDocument();
    expect(screen.queryByRole("textbox", { name: "Ask about this" })).not.toBeInTheDocument();
  });

  it("offers Try again on a failed answer, asking the same question", async () => {
    const mocked = api([{ body: MARTA_CHAT_FAILED }, { body: MARTA_CHAT_ANSWERED }], {
      [STREAM]: { stream: answerFrames(WHY_ANSWER) },
    });
    renderChat();

    expect(await screen.findByText("Scout couldn’t answer that.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));

    await waitFor(() => {
      expect(screen.queryByText("Scout couldn’t answer that.")).not.toBeInTheDocument();
    });
    expect(mocked.calls.find((call) => call.route === STREAM)?.body).toMatchObject({ text: WHY });
  });

  it("keeps the words in the field when the hour's questions are used", async () => {
    api([{ body: MARTA_CHAT }], { [STREAM]: problem(429, "rate_limited") });
    renderChat();
    const field = await screen.findByRole("textbox", { name: "Ask about this" });

    fireEvent.change(field, { target: { value: "When is it due?" } });
    fireEvent.keyDown(field, { key: "Enter" });

    expect(
      await screen.findByText("You asked a lot this hour. Ask again in a little while."),
    ).toBeInTheDocument();
    await waitFor(() => {
      expect(field).toHaveValue("When is it due?");
    });
  });

  it("offers Start over when the chat is full", async () => {
    const mocked = api([{ body: MARTA_CHAT }, { body: MARTA_CHAT }], {
      [STREAM]: problem(409, "chat_full"),
      [`DELETE ${CHATS}/${MARTA_CHAT.id}`]: { status: 204 },
    });
    renderChat();
    const field = await screen.findByRole("textbox", { name: "Ask about this" });

    fireEvent.change(field, { target: { value: "One more?" } });
    fireEvent.keyDown(field, { key: "Enter" });
    fireEvent.click(await screen.findByRole("button", { name: "Start over" }));

    await waitFor(() => {
      expect(mocked.calls.some((call) => call.route === `DELETE ${CHATS}/${MARTA_CHAT.id}`)).toBe(
        true,
      );
    });
  });

  it("closes on Esc", async () => {
    api([{ body: MARTA_CHAT }]);
    const { onClose } = renderChat();
    const field = await screen.findByRole("textbox", { name: "Ask about this" });

    fireEvent.keyDown(field, { key: "Escape" });

    expect(onClose).toHaveBeenCalledOnce();
  });

  describe("proposals", () => {
    const SETTLE = (id: string) => `POST ${CHATS}/${MARTA_CHAT.id}/proposals/${id}`;
    const ARCHIVE = `POST ${W}/conversations/${MARTA.id}/archive`;
    const UNARCHIVE = `POST ${W}/conversations/${MARTA.id}/unarchive`;

    /** The chat holding `proposals`, each settled as the client says. */
    function proposing(proposals = [ARCHIVE_PROPOSAL, RULE_PROPOSAL], more = {}) {
      const states = new Map<string, string>();
      const settle = (id: string) => (_request: Request, body: unknown) => {
        const state = (body as { state: string }).state;
        states.set(id, state);
        return { body: { ...proposals.find((p) => p.id === id), state } };
      };
      const keys: (string | null)[] = [];
      const mocked = mockApi({
        [`POST ${CHATS}`]: () => ({
          body: chatWithProposals(
            proposals.map((p) => ({ ...p, state: states.get(p.id) ?? p.state }) as typeof p),
          ),
        }),
        [`GET ${W}/conversations/${MARTA.id}`]: { body: MARTA_DETAIL },
        [`GET ${W}/conversations`]: { body: { conversations: [PEDRO], next_cursor: null } },
        [ARCHIVE]: (request) => {
          keys.push(request.headers.get("Idempotency-Key"));
          return { body: { ...MARTA, archived: true } };
        },
        [UNARCHIVE]: (request) => {
          keys.push(request.headers.get("Idempotency-Key"));
          return { body: MARTA };
        },
        ...Object.fromEntries(proposals.map((p) => [SETTLE(p.id), settle(p.id)])),
        ...more,
      });
      return { mocked, keys, states };
    }

    it("shows each proposal as a card from the record's facts, and sends nothing until Confirm", async () => {
      const { mocked } = proposing();
      renderChat();

      const card = await screen.findByRole("region", { name: "Archive" });
      expect(within(card).getByText("Marta Ferreira · Walkthrough on Thursday")).toBeVisible();
      expect(within(card).getByText("Scout proposes")).toBeVisible();
      const rule = screen.getByRole("region", { name: "A rule to teach Scout" });
      expect(within(rule).getByText(RULE_SENTENCE).tagName).toBe("Q");
      expect(mocked.calls.some((call) => call.route === ARCHIVE)).toBe(false);
    });

    it("confirms with the member's own request, keyed by the proposal, once; then Undo", async () => {
      const { mocked, keys, states } = proposing();
      renderChat();
      const card = await screen.findByRole("region", { name: "Archive" });

      const confirm = within(card).getByRole("button", { name: "Confirm" });
      fireEvent.click(confirm);
      fireEvent.click(confirm);

      expect(await within(card).findByText("Done.")).toBeVisible();
      expect(keys).toEqual([ARCHIVE_PROPOSAL.id]);
      await waitFor(() => {
        expect(states.get(ARCHIVE_PROPOSAL.id)).toBe("done");
      });
      fireEvent.click(within(card).getByRole("button", { name: "Undo" }));

      expect(await within(card).findByText("Undone.")).toBeVisible();
      expect(keys).toEqual([ARCHIVE_PROPOSAL.id, `${ARCHIVE_PROPOSAL.id}:undo`]);
      await waitFor(() => {
        expect(states.get(ARCHIVE_PROPOSAL.id)).toBe("undone");
      });
      expect(mocked.calls.filter((call) => call.route === ARCHIVE)).toHaveLength(1);
    });

    it("says it could not be done, with the API's reason, when the request is refused", async () => {
      const { states } = proposing(undefined, {
        [ARCHIVE]: {
          status: 409,
          body: {
            status: 409,
            code: "need_changed",
            title: "Changed",
            detail: "Marta wrote again since.",
          },
        },
      });
      renderChat();
      const card = await screen.findByRole("region", { name: "Archive" });

      fireEvent.click(within(card).getByRole("button", { name: "Confirm" }));

      expect(
        await within(card).findByText("This couldn’t be done: Marta wrote again since."),
      ).toBeVisible();
      await waitFor(() => {
        expect(states.get(ARCHIVE_PROPOSAL.id)).toBe("failed");
      });
      expect(within(card).queryByRole("button")).not.toBeInTheDocument();
    });

    it("never sends a request that is not one of its own actions", async () => {
      const odd = {
        ...ARCHIVE_PROPOSAL,
        request: {
          operation: "conversations.archive",
          method: "POST" as const,
          path: `../../workspaces/${DUARTE.id}`,
          body: null,
        },
      };
      const { mocked } = proposing([odd]);
      renderChat();

      const card = await screen.findByRole("region", { name: "Archive" });
      expect(within(card).getByText("Scout can’t do this here.")).toBeVisible();
      expect(within(card).queryByRole("button", { name: "Confirm" })).not.toBeInTheDocument();
      expect(mocked.calls.every((call) => !call.route.includes("workspaces"))).toBe(true);
    });

    it("settles Not now as declined, and a settled card has no buttons when read again", async () => {
      const { states } = proposing();
      renderChat();
      const card = await screen.findByRole("region", { name: "Archive" });

      fireEvent.click(within(card).getByRole("button", { name: "Not now" }));

      await waitFor(() => {
        expect(states.get(ARCHIVE_PROPOSAL.id)).toBe("declined");
      });
      expect(await within(card).findByText("Not now.")).toBeVisible();
      expect(within(card).queryByRole("button")).not.toBeInTheDocument();
    });

    it("opens Teach Scout with the rule's sentence, and settles done when the rule is saved", async () => {
      const inbox = MARTA_RULE.proposals[0]?.inboxes[0]?.id ?? "";
      const { mocked, states } = proposing(undefined, {
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
          body: { rule: { id: "rule-1" }, acted_without_asking: false },
        },
      });
      renderChat();
      const card = await screen.findByRole("region", { name: "A rule to teach Scout" });

      fireEvent.click(within(card).getByRole("button", { name: "Review the rule" }));

      const dialog = await screen.findByRole("dialog", { name: "Teach Scout something" });
      expect(within(dialog).getByRole("textbox", { name: "What should Scout do?" })).toHaveValue(
        RULE_SENTENCE,
      );
      await waitFor(() => {
        expect(
          mocked.calls.find((call) => call.route === `POST ${W}/assistant/compile`)?.body,
        ).toEqual({
          text: RULE_SENTENCE,
        });
      });
      // Nothing is saved until Sam presses Save rule.
      expect(mocked.calls.some((call) => call.route === `POST ${W}/inboxes/${inbox}/rules`)).toBe(
        false,
      );
      fireEvent.click(await within(dialog).findByRole("button", { name: /Save rule/u }));

      await waitFor(() => {
        expect(states.get(RULE_PROPOSAL.id)).toBe("done");
      });
      expect(await within(card).findByText("Rule saved.")).toBeVisible();
    });

    it("settles a rule card declined when Teach Scout closes without saving", async () => {
      const { states } = proposing(undefined, {
        [`POST ${W}/assistant/compile`]: { body: MARTA_RULE },
      });
      renderChat();
      const card = await screen.findByRole("region", { name: "A rule to teach Scout" });

      fireEvent.click(within(card).getByRole("button", { name: "Review the rule" }));
      const dialog = await screen.findByRole("dialog", { name: "Teach Scout something" });
      fireEvent.keyDown(dialog, { key: "Escape" });

      await waitFor(() => {
        expect(states.get(RULE_PROPOSAL.id)).toBe("declined");
      });
    });

    it("opens the Reply dialog with the intent through the app's opener; nothing is sent", async () => {
      const openReply = vi.fn();
      const { mocked, states } = proposing([REPLY_PROPOSAL]);
      renderChat(vi.fn(), openReply);
      const card = await screen.findByRole("region", { name: "A reply to Marta Ferreira" });
      expect(
        within(card).getByText("Confirm Thursday at 10 and ask who else is coming"),
      ).toBeVisible();

      fireEvent.click(within(card).getByRole("button", { name: "Write the draft" }));

      expect(openReply).toHaveBeenCalledWith({
        conversationId: MARTA.id,
        stream: "primary",
        name: "Marta Ferreira",
        intent: "Confirm Thursday at 10 and ask who else is coming",
      });
      await waitFor(() => {
        expect(states.get(REPLY_PROPOSAL.id)).toBe("done");
      });
      expect(mocked.calls.every((call) => !call.route.includes("/drafts"))).toBe(true);
    });
  });
  describe("about other things (phase 3)", () => {
    it("chats about a rule: its sentence, and questions about what it does", async () => {
      const mocked = api([{ status: 201, body: RULE_CHAT }], {
        [`GET ${W}/rules/${LEDGER_RULE.id}`]: { body: LEDGER_RULE },
      });
      renderChat(vi.fn(), undefined, {
        about: { type: "conversations.rule", id: LEDGER_RULE.id },
      });

      const region = await screen.findByRole("region", { name: "Scout" });
      expect(
        await within(region).findByText("Mail from hello@weekly-ledger.example → Newsletters"),
      ).toBeVisible();
      expect(within(region).getByText("Ask me about this rule.")).toBeVisible();
      for (const question of ["What does this rule do?", "What has it changed?"]) {
        expect(within(region).getByRole("button", { name: question })).toBeVisible();
      }
      expect(mocked.calls.find((call) => call.route === `POST ${CHATS}`)?.body).toEqual({
        about: { type: "conversations.rule", id: LEDGER_RULE.id },
      });
    });

    it("chats about one of Scout's suggestions: its sender, and why", async () => {
      const stream = `POST ${CHATS}/${NOTICE_CHAT.id}/questions/stream`;
      const mocked = api(
        [
          { status: 201, body: NOTICE_CHAT },
          { body: answered(NOTICE_CHAT, NOTICE_WHY, NOTICE_ANSWER) },
        ],
        {
          [`GET ${W}/rule-notices/${BUILDS_NOTICE.id}`]: { body: BUILDS_NOTICE },
          [stream]: { stream: answerFrames(NOTICE_ANSWER, "CI writes most days.") },
        },
      );
      renderChat(vi.fn(), undefined, {
        about: { type: "conversations.rule_notice", id: BUILDS_NOTICE.id },
      });

      expect(
        await screen.findByText("Scout’s suggestion for CI (builds@ci.example.com)"),
      ).toBeVisible();
      fireEvent.click(await screen.findByRole("button", { name: NOTICE_WHY }));

      expect(await screen.findAllByText(/you opened 2 of them/u)).not.toHaveLength(0);
      expect(mocked.calls.find((call) => call.route === stream)?.body).toMatchObject({
        text: NOTICE_WHY,
      });
      expect(screen.queryByRole("button", { name: "What would it change?" })).toBeNull();
    });

    it("says a suggestion is gone when it is no longer offered", async () => {
      api([problem(404, "not_found")]);
      renderChat(vi.fn(), undefined, {
        about: { type: "conversations.rule_notice", id: BUILDS_NOTICE.id },
      });

      expect(await screen.findByText("This suggestion is no longer offered to you.")).toBeVisible();
      expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    });

    it("starts a chat about nothing and asks the question handed over, once", async () => {
      const stream = `POST ${CHATS}/${NOTHING_CHAT.id}/questions/stream`;
      const mocked = api([{ status: 201, body: NOTHING_CHAT }], {
        [stream]: { stream: answerFrames(WAITING_ANSWER, "Two people owe Sam an answer.") },
        [`GET ${CHATS}/${NOTHING_CHAT.id}`]: {
          body: answered(NOTHING_CHAT, WAITING_QUESTION, WAITING_ANSWER),
        },
      });
      renderChat(vi.fn(), undefined, { about: null, opening: 1, ask: WAITING_QUESTION });

      expect(await screen.findAllByText(/You’re waiting on two people/u)).not.toHaveLength(0);
      expect(screen.getByText("your mail")).toBeVisible();
      expect(mocked.calls.find((call) => call.route === `POST ${CHATS}`)?.body).toEqual({});
      await waitFor(() => {
        // Read again by its id: starting again would make another chat.
        expect(mocked.calls.some((call) => call.route === `GET ${CHATS}/${NOTHING_CHAT.id}`)).toBe(
          true,
        );
      });
      expect(mocked.calls.filter((call) => call.route === stream)).toHaveLength(1);
      expect(mocked.calls.filter((call) => call.route === `POST ${CHATS}`)).toHaveLength(1);
      expect(screen.getByRole("textbox", { name: "Ask about your mail" })).toBeVisible();
    });

    it("offers what needs you and what you wait on in a chat about nothing", async () => {
      api([{ status: 201, body: NOTHING_CHAT }]);
      renderChat(vi.fn(), undefined, { about: null, opening: 2 });

      for (const question of ["What needs me today?", WAITING_QUESTION]) {
        expect(await screen.findByRole("button", { name: question })).toBeVisible();
      }
    });

    it("lists Recent by first question and reopens a chat chosen there", async () => {
      const mocked = api([{ status: 201, body: NOTHING_CHAT }], {
        [`GET ${CHATS}`]: { body: { chats: RECENT } },
        [`GET ${CHATS}/${MARTA_CHAT.id}`]: { body: MARTA_CHAT_ANSWERED },
      });
      renderChat(vi.fn(), undefined, { about: null, opening: 3 });
      await screen.findByRole("button", { name: "What needs me today?" });

      fireEvent.click(screen.getByRole("button", { name: "Recent" }));
      const menu = await screen.findByRole("dialog", { name: "Recent" });
      const rows = await within(menu).findAllByRole("button");
      expect(rows.map((row) => row.querySelector("span")?.textContent)).toEqual([
        WAITING_QUESTION,
        WHY,
        "What has it changed?",
      ]);
      expect(rows[0]).toHaveAttribute("aria-current", "true");
      const waiting = within(menu).getByRole("button", { name: new RegExp(WAITING_QUESTION, "u") });
      const why = within(menu).getByRole("button", { name: new RegExp(WHY, "u") });
      waiting.focus();
      fireEvent.keyDown(waiting, { key: "ArrowDown" });
      expect(why).toHaveFocus();

      fireEvent.click(why);
      expect(
        await screen.findAllByText(/It is under Reply because Marta asks you/u),
      ).not.toHaveLength(0);
      expect(await screen.findByText("Marta Ferreira · Walkthrough on Thursday")).toBeVisible();
      expect(mocked.calls.some((call) => call.route === `GET ${CHATS}/${MARTA_CHAT.id}`)).toBe(
        true,
      );
    });

    it("closes Recent with Esc before the panel", async () => {
      api([{ status: 201, body: NOTHING_CHAT }], { [`GET ${CHATS}`]: { body: { chats: [] } } });
      const { onClose } = renderChat(vi.fn(), undefined, { about: null, opening: 4 });
      await screen.findByRole("button", { name: "What needs me today?" });

      fireEvent.click(screen.getByRole("button", { name: "Recent" }));
      const menu = await screen.findByRole("dialog", { name: "Recent" });
      expect(
        await within(menu).findByText("No chats yet. Ask Scout something to start one."),
      ).toBeVisible();
      fireEvent.keyDown(menu, { key: "Escape" });

      expect(onClose).not.toHaveBeenCalled();
      expect(screen.queryByText("No chats yet. Ask Scout something to start one.")).toBeNull();
    });
  });
});
