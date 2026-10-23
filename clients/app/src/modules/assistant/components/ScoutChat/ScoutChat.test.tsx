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

import {
  answerFrames,
  MARTA_CHAT,
  MARTA_CHAT_ANSWERED,
  MARTA_CHAT_FAILED,
  WHY,
  WHY_ANSWER,
} from "../../../../../test/demo/assistant";
import { MARTA, MARTA_DETAIL, PEDRO } from "../../../../../test/demo/attention";
import { W } from "../../../../../test/demo/conversations";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { ScoutChat } from "./ScoutChat";

const CHATS = `${W}/assistant/chats`;
const STREAM = `POST ${CHATS}/${MARTA_CHAT.id}/questions/stream`;

function renderChat(onClose = vi.fn()) {
  const root = createRootRoute({ component: Outlet });
  const page = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId",
    component: () => (
      <ScoutChat
        workspaceId={DUARTE.id}
        conversationId={MARTA.id}
        onClose={onClose}
        modal={false}
      />
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
    expect(
      screen.getByText("Scout looks things up and explains. In this version it changes nothing."),
    ).toBeInTheDocument();
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
    expect(within(answer).getAllByRole("listitem")).toHaveLength(2);
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
});
