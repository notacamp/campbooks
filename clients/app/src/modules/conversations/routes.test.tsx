import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import {
  INBOX,
  LEDGER,
  LEDGER_DETAIL,
  LEDGER_RULE,
  LEDGER_SUGGESTIONS,
  ledgerBody,
  PRIMARY,
  Q4,
  Q4_BODY,
  Q4_DETAIL,
  STREAM_COUNTS,
  W,
} from "../../../test/demo/conversations";
import { DUARTE } from "../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../test/render";

import { buildConversationsRoutes } from "./index";

function renderAt(path: string, locale: "en" | "pt-PT" = "en") {
  const root = createRootRoute({ component: Outlet });
  const workspace = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId",
    component: Outlet,
  });
  const router = createRouter({
    routeTree: root.addChildren([workspace.addChildren(buildConversationsRoutes(workspace))]),
    history: createMemoryHistory({ initialEntries: [path] }),
  });
  return { router, ...renderWithProviders(<RouterProvider router={router} />, locale) };
}

const summary = (id: string) => PRIMARY.find((conversation) => conversation.id === id);

function inbox(extra: Parameters<typeof mockApi>[0] = {}) {
  return mockApi({
    [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
    [`GET ${W}/streams`]: { body: { streams: STREAM_COUNTS } },
    [`GET ${W}/conversations`]: (request) => {
      const stream = new URL(request.url).searchParams.get("stream");
      return {
        body: {
          conversations: stream === "primary" ? PRIMARY : [],
          next_cursor: null,
        },
      };
    },
    [`GET ${W}/conversations/${Q4.id}`]: { body: Q4_DETAIL },
    [`GET ${W}/conversations/${LEDGER.id}`]: { body: LEDGER_DETAIL },
    [`GET ${W}/conversations/${Q4.id}/messages/${Q4_DETAIL.messages[2]?.id ?? ""}/body`]: {
      body: Q4_BODY,
    },
    [`GET ${W}/conversations/${LEDGER.id}/messages/${LEDGER_DETAIL.messages[0]?.id ?? ""}/body`]: (
      request,
    ) => ({ body: ledgerBody(new URL(request.url).searchParams.get("show_images") === "true") }),
    [`POST ${W}/conversations/${Q4.id}/read`]: { body: { ...Q4, unread_count: 0 } },
    [`POST ${W}/conversations/${LEDGER.id}/read`]: { body: { ...LEDGER, unread_count: 0 } },
    [`POST ${W}/conversations/${Q4.id}/archive`]: { body: { ...Q4, archived: true } },
    [`POST ${W}/conversations/${Q4.id}/unarchive`]: { body: Q4 },
    [`POST ${W}/conversations/${LEDGER.id}/stream`]: (_request, body) => ({
      body: { ...LEDGER, ...(body as object), stream_source: "member", suggestion: null },
    }),
    ...extra,
  });
}

describe("the inbox", () => {
  it("lists a stream newest first, with its suggestion shown as a suggestion", async () => {
    inbox();
    renderAt(`/w/${DUARTE.id}/inbox/primary`);

    expect(await screen.findByRole("heading", { level: 1, name: "Primary" })).toBeVisible();
    const list = screen.getByRole("region", { name: "Conversations" });
    expect(await within(list).findByText(Q4.subject)).toBeVisible();
    expect(within(list).getByText("Suggests Newsletters")).toBeVisible();
    expect(screen.getByRole("heading", { name: "Pick a conversation" })).toBeVisible();
  });

  it("goes to Primary from an unknown stream", async () => {
    inbox();
    const { router } = renderAt(`/w/${DUARTE.id}/inbox/spam`);

    await waitFor(() => {
      expect(router.state.location.pathname).toBe(`/w/${DUARTE.id}/inbox/primary`);
    });
  });

  it("opens a conversation: newest message open, older folded, read on open", async () => {
    const api = inbox();
    renderAt(`/w/${DUARTE.id}/inbox/primary/${Q4.id}`);

    const reader = await screen.findByRole("main", { name: "Conversation" });
    expect(await within(reader).findByRole("heading", { name: Q4.subject })).toBeVisible();
    // The newest message is open, in the sandboxed frame.
    const frame = await within(reader).findByTitle("Message from Maya Okafor");
    expect(frame.getAttribute("sandbox")).toBe("allow-popups allow-popups-to-escape-sandbox");
    expect(within(reader).getByText("q4-plan-v3.pdf")).toBeVisible();
    // The two older ones are folded until opened.
    const folded = within(reader).getAllByRole("button", { expanded: false });
    expect(folded.length).toBeGreaterThanOrEqual(2);
    fireEvent.click(within(reader).getByRole("button", { name: /Sam Rivera/u }));
    expect(await within(reader).findByText(/Is the timeline fixed\?/u)).toBeVisible();
    // The note says what the decide pass thinks, as suggestions; how it
    // decided (the only place with numbers) is folded away.
    const note = within(reader).getByRole("region", { name: "Where things stand" });
    expect(note).toHaveTextContent("it needs a reply and it mentions a date to meet");
    expect(note).not.toHaveTextContent("%");
    fireEvent.click(within(note).getByText("How Campbooks decided"));
    expect(within(note).getByText("Yes. It names a date (“Friday”).")).toBeVisible();
    await waitFor(() => {
      expect(api.calls.map((call) => call.route)).toContain(
        `POST ${W}/conversations/${Q4.id}/read`,
      );
    });
  });

  it("archives with E and takes it back with Z", async () => {
    const api = inbox();
    renderAt(`/w/${DUARTE.id}/inbox/primary/${Q4.id}`);
    await screen.findByRole("heading", { name: Q4.subject });

    fireEvent.keyDown(document, { key: "e" });
    expect(await screen.findByText(`Done · ${Q4.subject}`)).toBeVisible();
    await waitFor(() => {
      expect(api.calls.map((call) => call.route)).toContain(
        `POST ${W}/conversations/${Q4.id}/archive`,
      );
    });
    fireEvent.keyDown(document, { key: "z" });
    await waitFor(() => {
      expect(api.calls.map((call) => call.route)).toContain(
        `POST ${W}/conversations/${Q4.id}/unarchive`,
      );
    });
  });

  it("takes a suggestion with 1, and shows images only when asked", async () => {
    const api = inbox();
    renderAt(`/w/${DUARTE.id}/inbox/primary/${LEDGER.id}`);
    const reader = await screen.findByRole("main", { name: "Conversation" });
    await within(reader).findByRole("heading", { name: LEDGER.subject });

    const frame = await within(reader).findByTitle("Message from The Weekly Ledger");
    expect(frame.getAttribute("srcdoc")).toContain("img-src data:;");
    expect(within(reader).getByText(/1 remote image hidden/u)).toBeVisible();
    fireEvent.click(within(reader).getByRole("button", { name: /Show images/u }));
    await waitFor(() => {
      expect(
        within(reader).getByTitle("Message from The Weekly Ledger").getAttribute("srcdoc"),
      ).toContain("img-src data: https: http:");
    });

    fireEvent.keyDown(document, { key: "1" });
    expect(await screen.findByText(`Moved to Newsletters · ${LEDGER.subject}`)).toBeVisible();
    const moved = api.calls.find(
      (call) => call.route === `POST ${W}/conversations/${LEDGER.id}/stream`,
    );
    expect(moved?.body).toEqual({ stream: "newsletters" });
  });

  it("offers to do the same to every lookalike, then shows the rule's run with Undo all", async () => {
    const api = inbox({
      [`GET ${W}/conversations/${LEDGER.id}/rule-suggestions`]: { body: LEDGER_SUGGESTIONS },
      [`POST ${W}/inboxes/${INBOX.id}/rules`]: {
        status: 201,
        body: { rule: LEDGER_RULE, acted_without_asking: false },
      },
      [`GET ${W}/rules/${LEDGER_RULE.id}`]: { body: LEDGER_RULE },
      [`GET ${W}/rules`]: { body: { rules: [LEDGER_RULE] } },
      [`DELETE ${W}/rules/${LEDGER_RULE.id}`]: { body: { ...LEDGER_RULE, state: "removed" } },
    });
    renderAt(`/w/${DUARTE.id}/inbox/primary/${LEDGER.id}`);
    const reader = await screen.findByRole("main", { name: "Conversation" });
    await within(reader).findByRole("heading", { name: LEDGER.subject });

    // Move it (the suggestion's 1), and the offer asks about the 40 like it.
    fireEvent.keyDown(document, { key: "1" });
    const offer = await screen.findByRole("dialog", {
      name: "Move all 40 like this to Newsletters, and new ones too?",
    });
    expect(within(offer).getByText("What changes for freelancers in 2027")).toBeVisible();
    expect(within(offer).getByText("None of them looks suspicious.")).toBeVisible();

    // 1 accepts: the rule is made from the chosen likeness.
    fireEvent.keyDown(document, { key: "1" });
    expect(
      await screen.findByRole("dialog", {
        name: "Moved 40 to Newsletters. New ones will go there too.",
      }),
    ).toBeVisible();
    const made = api.calls.find((call) => call.route === `POST ${W}/inboxes/${INBOX.id}/rules`);
    expect(made?.body).toMatchObject({
      conditions: LEDGER_SUGGESTIONS.suggestions[0]?.conditions,
      actions: [{ action: "move_to_stream", stream: "newsletters" }],
      apply_to_existing: true,
      from_conversation_id: LEDGER.id,
    });

    // Undo all puts them back.
    fireEvent.click(screen.getByRole("button", { name: "Undo all" }));
    await waitFor(() => {
      expect(
        api.calls.some((call) => call.route === `DELETE ${W}/rules/${LEDGER_RULE.id}`),
      ).toBe(true);
    });
  });

  it("forgets the offer for this mail with Just this one", async () => {
    const api = inbox({
      [`GET ${W}/conversations/${LEDGER.id}/rule-suggestions`]: { body: LEDGER_SUGGESTIONS },
      [`POST ${W}/conversations/${LEDGER.id}/rule-suggestions/dismiss`]: { status: 204 },
    });
    renderAt(`/w/${DUARTE.id}/inbox/primary/${LEDGER.id}`);
    const reader = await screen.findByRole("main", { name: "Conversation" });
    await within(reader).findByRole("heading", { name: LEDGER.subject });
    fireEvent.keyDown(document, { key: "1" });
    await screen.findByRole("dialog", { name: /Move all 40/u });
    fireEvent.click(screen.getByRole("button", { name: /Just this one/u }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: /Move all 40/u })).toBeNull();
    });
    const dismissed = api.calls.find(
      (call) => call.route === `POST ${W}/conversations/${LEDGER.id}/rule-suggestions/dismiss`,
    );
    expect(dismissed?.body).toEqual({ key: "sender|ledger" });
  });

  it("says when a conversation is gone, and when the list cannot load", async () => {
    inbox({
      [`GET ${W}/conversations/${Q4.id}`]: problem(404, "not_found"),
      [`GET ${W}/conversations`]: problem(503, "unavailable"),
    });
    renderAt(`/w/${DUARTE.id}/inbox/primary/${Q4.id}`);

    expect(await screen.findByRole("heading", { name: "This conversation is gone" })).toBeVisible();
    expect(await screen.findByRole("heading", { name: "Could not load Primary" })).toBeVisible();
    expect(summary(Q4.id)).toBeDefined();
  });

  it("renders in the user's language", async () => {
    inbox();
    renderAt(`/w/${DUARTE.id}/inbox/primary`, "pt-PT");

    expect(await screen.findByRole("heading", { level: 1, name: "Principal" })).toBeVisible();
    expect(await screen.findByText("Sugere Newsletters")).toBeVisible();
  });
});
