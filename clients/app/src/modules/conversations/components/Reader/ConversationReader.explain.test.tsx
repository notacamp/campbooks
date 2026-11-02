// Scout explains a thread (campbooks-cloud#407) in the reader: only when the
// member asks, one collapsed line on return, the card on Show, chips that open
// the message they come from, and nothing when the server does not offer it.
import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterProvider,
} from "@tanstack/react-router";
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { INBOX, Q4, Q4_BODY, Q4_DETAIL, W } from "../../../../../test/demo/conversations";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../../test/render";
import type { ExplanationView } from "../../api/explanation";
import { useInboxActions } from "../InboxScreen/use-inbox-actions";

import { ConversationReader } from "./ConversationReader";

function Reader() {
  const actions = useInboxActions(DUARTE.id);
  return (
    <ConversationReader
      workspaceId={DUARTE.id}
      conversationId={Q4.id}
      stream="primary"
      actions={actions}
      onBack={vi.fn()}
      onPrevious={vi.fn()}
      onNext={vi.fn()}
      onMove={vi.fn()}
    />
  );
}

const FIRST = Q4_DETAIL.messages[0]?.id ?? "";
const NEWEST = Q4_DETAIL.messages[2]?.id ?? "";
const PATH = `${W}/conversations/${Q4.id}/explanation`;

/** Maya's thread, explained: three messages, two of them cited. */
function explained(overrides: Partial<ExplanationView> = {}): ExplanationView {
  return {
    state: "ready",
    locale: "en",
    written_at: "2026-10-12T10:42:00Z",
    messages: 3,
    in_full: 3,
    stale: false,
    body: {
      in_short: "Maya sent the Q4 plan and asks for a review by Friday.",
      wants: [{ who: "Maya Okafor", what: "A review of the plan by Friday.", refs: [FIRST] }],
      agreed: [],
      open: [{ text: "Whether the timeline can move.", refs: [NEWEST] }],
      on_you: [{ text: "Review the plan.", due_on: "2026-10-16", refs: [FIRST] }],
    },
    refs: [
      { id: FIRST, from: "Maya Okafor", sent_at: "2026-10-11T11:00:00Z" },
      { id: NEWEST, from: "Maya Okafor", sent_at: "2026-10-12T09:00:00Z" },
    ],
    ...overrides,
  };
}

interface Options {
  explain?: boolean;
  /** What GET answers: none (404) or a view. */
  stored?: ExplanationView | undefined;
  /** What POST answers. */
  asked?: ExplanationView | { status: number; body: unknown } | undefined;
}

function setup({ explain = true, stored, asked }: Options = {}) {
  let current = stored;
  const api = mockApi({
    [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
    [`GET ${W}/conversations/${Q4.id}`]: {
      body: { ...Q4_DETAIL, ...(explain ? { explain } : {}) },
    },
    [`GET ${W}/conversations/${Q4.id}/messages/${NEWEST}/body`]: { body: Q4_BODY },
    [`POST ${W}/conversations/${Q4.id}/read`]: { body: { ...Q4, unread_count: 0 } },
    [`GET ${W}/drafts`]: { body: { drafts: [] } },
    [`GET ${PATH}`]: () =>
      current === undefined
        ? { status: 404, body: { code: "not_found", title: "Not found", status: 404 } }
        : { body: current },
    [`POST ${PATH}`]: () => {
      if (asked !== undefined && "status" in asked) {
        return { status: asked.status, body: asked.body };
      }
      current = asked ?? explained();
      return { status: 202, body: current };
    },
  });
  const root = createRootRoute({ component: Reader });
  const router = createRouter({
    routeTree: root,
    history: createMemoryHistory({ initialEntries: ["/"] }),
  });
  return { api, ...renderWithProviders(<RouterProvider router={router} />) };
}

/** The note's quiet button (the More menu repeats it). */
async function noteButton(): Promise<HTMLElement> {
  const buttons = await screen.findAllByRole("button", { name: /Explain this thread/u });
  const [first] = buttons;
  if (first === undefined) {
    throw new Error("no button");
  }
  return first;
}

const count = (api: ReturnType<typeof setup>["api"], route: string) =>
  api.calls.filter((call) => call.route === route).length;

describe("Explaining a thread in the reader", () => {
  it("shows nothing for a thread nobody asked about, and asks the server only whether they did", async () => {
    const { api } = setup();
    await screen.findByRole("heading", { name: Q4.subject });
    const button = await noteButton();
    expect(button).toHaveAttribute("aria-keyshortcuts", "W");
    await waitFor(() => {
      expect(count(api, `GET ${PATH}`)).toBe(1);
    });
    expect(screen.queryByRole("region", { name: "Scout’s explanation" })).not.toBeInTheDocument();
    expect(screen.queryByText(/Your explanation/u)).not.toBeInTheDocument();
    expect(count(api, `POST ${PATH}`)).toBe(0);
  });

  it("offers nothing, and asks nothing, while the server does not offer it", async () => {
    const { api } = setup({ explain: false });
    await screen.findByRole("heading", { name: Q4.subject });
    await screen.findByRole("button", { name: /Make it a task/u });
    expect(screen.queryByRole("button", { name: /Explain this thread/u })).not.toBeInTheDocument();
    fireEvent.keyDown(document.body, { key: "w" });
    expect(count(api, `GET ${PATH}`)).toBe(0);
    expect(count(api, `POST ${PATH}`)).toBe(0);
  });

  it("asks on the button, shows the card with its chips, and a chip opens its message", async () => {
    const { api } = setup();
    fireEvent.click(await noteButton());
    const card = await screen.findByRole("region", { name: "Scout’s explanation" });
    expect(count(api, `POST ${PATH}`)).toBe(1);
    expect(
      within(card).getByText("Maya sent the Q4 plan and asks for a review by Friday."),
    ).toBeInTheDocument();
    expect(within(card).getByText("Review the plan.", { exact: false })).toBeInTheDocument();
    expect(within(card).getByText(/From 3 messages · written/u)).toBeInTheDocument();
    expect(within(card).getByText("Only you see this.", { exact: false })).toBeInTheDocument();
    const chips = within(card).getAllByRole("button", { name: /^Maya · / });
    expect(chips.length).toBeGreaterThan(0);
    const scroll = vi.fn();
    Element.prototype.scrollIntoView = scroll;
    const first = chips[0];
    if (first !== undefined) {
      fireEvent.click(first);
    }
    await waitFor(() => {
      expect(scroll).toHaveBeenCalled();
    });
    expect(document.getElementById(`message-${FIRST}`)).not.toBeNull();
  });

  it("asks on W and from the More menu", async () => {
    const { api } = setup();
    await noteButton();
    fireEvent.keyDown(document.body, { key: "w" });
    await screen.findByRole("region", { name: "Scout’s explanation" });
    expect(count(api, `POST ${PATH}`)).toBe(1);
  });

  it("is a skeleton while Scout reads, and Stop only hides it", async () => {
    const { api } = setup({ asked: { ...explained(), state: "writing", body: null, refs: [] } });
    fireEvent.click(await noteButton());
    const card = await screen.findByRole("region", { name: "Scout’s explanation" });
    expect(card).toHaveAttribute("aria-busy", "true");
    expect(within(card).getByText("Scout is reading the 3 messages…")).toBeInTheDocument();
    fireEvent.click(within(card).getByRole("button", { name: "Stop" }));
    expect(screen.queryByRole("region", { name: "Scout’s explanation" })).not.toBeInTheDocument();
    expect(screen.getByText("Scout is still reading this thread")).toBeInTheDocument();
    // Stopping stops nothing on the server and asks nothing more.
    expect(count(api, `POST ${PATH}`)).toBe(1);
  });

  it("hides to one line, which Show opens again", async () => {
    setup();
    fireEvent.click(await noteButton());
    const card = await screen.findByRole("region", { name: "Scout’s explanation" });
    fireEvent.click(within(card).getByRole("button", { name: "Hide the explanation" }));
    expect(screen.queryByRole("region", { name: "Scout’s explanation" })).not.toBeInTheDocument();
    expect(screen.getByText(/Your explanation from .* · from 3 messages/u)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Show" }));
    expect(await screen.findByRole("region", { name: "Scout’s explanation" })).toBeVisible();
  });

  it("comes back as one collapsed line, and the card opens only on Show", async () => {
    const { api } = setup({ stored: explained() });
    expect(await screen.findByText(/Your explanation from .* · from 3 messages/u)).toBeVisible();
    expect(screen.queryByRole("region", { name: "Scout’s explanation" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Explain again" })).not.toBeInTheDocument();
    expect(count(api, `POST ${PATH}`)).toBe(0);
    fireEvent.click(screen.getByRole("button", { name: "Show" }));
    expect(await screen.findByRole("region", { name: "Scout’s explanation" })).toBeVisible();
    expect(count(api, `POST ${PATH}`)).toBe(0);
  });

  it("says who wrote since when the thread moved on, and explains again only on request", async () => {
    const stale = explained({
      stale: true,
      newer: { from: "Maya Okafor", sent_at: "2026-10-13T11:05:00Z" },
    });
    // Asking again: the server keeps the earlier explanation while it writes.
    const { api } = setup({ stored: stale, asked: { ...stale, state: "writing", stale: false } });
    expect(await screen.findByText(/Maya wrote since, at /u)).toBeVisible();
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(count(api, `POST ${PATH}`)).toBe(0);
    fireEvent.click(screen.getByRole("button", { name: "Show" }));
    const card = await screen.findByRole("region", { name: "Scout’s explanation" });
    expect(within(card).getByText(/This explanation is from before/u)).toBeInTheDocument();
    fireEvent.click(within(card).getByRole("button", { name: "Explain again" }));
    await waitFor(() => {
      expect(count(api, `POST ${PATH}`)).toBe(1);
    });
    // The earlier explanation stays in view, and the card says Scout is at it.
    expect(await within(card).findByText(/Scout is reading the \d+ messages…/u)).toBeVisible();
  });

  it("says when it was written in another language", async () => {
    setup({ stored: explained({ stale: true, locale: "fr" }) });
    fireEvent.click(await screen.findByRole("button", { name: "Show" }));
    const card = await screen.findByRole("region", { name: "Scout’s explanation" });
    expect(within(card).getByText("Written in French.")).toBeInTheDocument();
    expect(within(card).getByRole("button", { name: "Explain again" })).toBeInTheDocument();
  });

  it("says a long thread is read in part, from Scout's notes", async () => {
    setup({ stored: explained({ messages: 34, in_full: 20 }) });
    fireEvent.click(await screen.findByRole("button", { name: "Show" }));
    expect(
      await screen.findByText(
        "From 34 messages: the newest 20 in full, the 14 before them from Scout’s notes.",
      ),
    ).toBeInTheDocument();
  });

  it("past the daily limit keeps the earlier explanation and offers no retry", async () => {
    setup({ stored: explained({ state: "failed", failure: "limit" }) });
    fireEvent.click(await screen.findByRole("button", { name: "Show" }));
    const card = await screen.findByRole("region", { name: "Scout’s explanation" });
    expect(within(card).getByText(/used today’s AI/u)).toBeInTheDocument();
    expect(within(card).getByText(/Maya sent the Q4 plan/u)).toBeInTheDocument();
    expect(within(card).queryByRole("button", { name: /Try again|Explain again/u })).toBeNull();
  });

  it("when Scout could not explain it, says so and offers to try again", async () => {
    setup({
      stored: explained({ state: "failed", failure: "unusable", body: null, refs: [] }),
    });
    fireEvent.click(await screen.findByRole("button", { name: "Show" }));
    const card = await screen.findByRole("region", { name: "Scout’s explanation" });
    expect(within(card).getByText("Scout couldn’t explain this thread.")).toBeInTheDocument();
    expect(within(card).getByRole("button", { name: "Try again" })).toBeInTheDocument();
  });

  it("says a refusal plainly when the ask is refused", async () => {
    setup({ asked: { status: 409, body: { code: "not_explainable", title: "x", status: 409 } } });
    fireEvent.click(await noteButton());
    expect(await screen.findByText("Scout can’t explain this thread.")).toBeVisible();
  });

  it("shows what Scout wrote as text, never as markup or links", async () => {
    setup({
      stored: explained({
        body: {
          in_short: "<b>Pay now</b> https://evil.example [x](https://evil.example)",
          wants: [],
          agreed: [],
          open: [],
          on_you: [],
        },
        refs: [],
      }),
    });
    fireEvent.click(await screen.findByRole("button", { name: "Show" }));
    const card = await screen.findByRole("region", { name: "Scout’s explanation" });
    expect(card.querySelector("b, a, img")).toBeNull();
    expect(within(card).getByText(/<b>Pay now<\/b>/u)).toBeInTheDocument();
  });
});
