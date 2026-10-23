// A reader without the send slice's composer offers no Reply: none that
// would do nothing (campbooks-cloud#220).
import { fireEvent, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ScoutChatProvider } from "~/lib/scout-chat";

import { INBOX, Q4, Q4_BODY, Q4_DETAIL, W } from "../../../../../test/demo/conversations";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../../test/render";
import { useInboxActions } from "../InboxScreen/use-inbox-actions";

import { ConversationReader } from "./ConversationReader";
import type { ConversationSummary } from "../../api/queries";
import type { Move } from "../../model";

import type { RenderReplySlot } from "./ReplySlot";

type OnMove = (conversation: ConversationSummary, move: Move) => void;

function Reader({ slot, onMove = vi.fn() }: { slot?: RenderReplySlot; onMove?: OnMove }) {
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
      onMove={onMove}
      renderReplySlot={slot}
    />
  );
}

const NEWEST = Q4_DETAIL.messages[2]?.id ?? "";

function routes(conversation: ConversationSummary = Q4) {
  return mockApi({
    [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
    [`GET ${W}/conversations/${Q4.id}`]: { body: { ...Q4_DETAIL, conversation } },
    // In spam the server holds remote images back, whoever asks.
    [`GET ${W}/conversations/${Q4.id}/messages/${NEWEST}/body`]: {
      body: conversation.spam ? { ...Q4_BODY, remote_images: 2, images_held: "spam" } : Q4_BODY,
    },
    [`POST ${W}/conversations/${Q4.id}/read`]: { body: { ...Q4, unread_count: 0 } },
    [`GET ${W}/drafts`]: { body: { drafts: [] } },
  });
}

describe("ConversationReader", () => {
  it("offers Ask Scout on ., opening the chat about this conversation", async () => {
    routes();
    const scout = vi.fn();
    renderWithProviders(
      <ScoutChatProvider open={scout}>
        <Reader />
      </ScoutChatProvider>,
    );
    await screen.findByRole("heading", { name: Q4.subject });
    const [ask] = screen.getAllByRole("button", { name: /Ask Scout/u });
    if (ask === undefined) {
      throw new Error("no Ask Scout");
    }
    expect(ask).toHaveAttribute("aria-keyshortcuts", ".");

    // The focus comes back to the button it was asked from.
    ask.focus();
    fireEvent.click(ask);
    expect(scout).toHaveBeenCalledWith(Q4.id, ask);
    fireEvent.keyDown(document.body, { key: "." });
    expect(scout).toHaveBeenCalledTimes(2);
    expect(scout.mock.calls[1]?.[0]).toBe(Q4.id);
  });

  it("offers no Ask Scout where no chat is mounted", async () => {
    routes();
    renderWithProviders(<Reader />);
    await screen.findByRole("heading", { name: Q4.subject });

    expect(screen.queryByRole("button", { name: /Ask Scout/u })).toBeNull();
  });

  it("offers no Reply, Reply all or Forward without a composer, and R does nothing", async () => {
    routes();
    renderWithProviders(<Reader />);
    await screen.findByRole("heading", { name: Q4.subject });
    for (const name of [/^Reply/u, /^Forward/u]) {
      expect(screen.queryByRole("button", { name })).toBeNull();
    }
    fireEvent.keyDown(document.body, { key: "r" });
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("offers Reply with a composer, and it opens the reply dialog", async () => {
    routes();
    const slot = vi.fn<RenderReplySlot>(() => null);
    renderWithProviders(<Reader slot={slot} />);
    await screen.findByRole("heading", { name: Q4.subject });
    const [reply] = await screen.findAllByRole("button", { name: /^Reply(?! all)/u });
    if (reply === undefined) {
      throw new Error("no Reply");
    }
    fireEvent.click(reply);
    expect(await screen.findByRole("dialog")).toBeVisible();
  });

  it("puts Reply in the header beside Reply all and Forward, not at the bottom", async () => {
    routes();
    const slot = vi.fn<RenderReplySlot>(() => null);
    renderWithProviders(<Reader slot={slot} />);
    await screen.findByRole("heading", { name: Q4.subject });
    // The header's actions are icons named by aria-label; the phone bar's Reply is not.
    const icons = screen
      .getAllByRole("button")
      .map((button) => button.getAttribute("aria-label"))
      .filter((label) => label !== null);
    const at = icons.indexOf("Reply");
    expect(icons.slice(at, at + 3)).toEqual(["Reply", "Reply all", "Forward"]);
    expect(screen.queryByRole("button", { name: /^Reply to/u })).toBeNull();
  });

  it("offers Reply in the note when it says a reply is needed, opening the same dialog", async () => {
    routes();
    const slot = vi.fn<RenderReplySlot>(() => null);
    renderWithProviders(<Reader slot={slot} />);
    await screen.findByRole("heading", { name: Q4.subject });
    const note = await screen.findByRole("region", { name: "Where things stand" });
    expect(note).toHaveTextContent("it needs a reply");
    fireEvent.click(within(note).getByRole("button", { name: "Reply" }));
    expect(await screen.findByRole("dialog")).toBeVisible();
  });

  it("offers no Reply in the note when it does not say a reply is needed", async () => {
    const detail = {
      ...Q4_DETAIL,
      conversation: Q4,
      decisions: Q4_DETAIL.decisions.map((d) =>
        d.question === "needs_reply" ? { ...d, value: false } : d,
      ),
    };
    mockApi({
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
      [`GET ${W}/conversations/${Q4.id}`]: { body: detail },
      [`GET ${W}/conversations/${Q4.id}/messages/${NEWEST}/body`]: { body: Q4_BODY },
      [`POST ${W}/conversations/${Q4.id}/read`]: { body: { ...Q4, unread_count: 0 } },
      [`GET ${W}/drafts`]: { body: { drafts: [] } },
    });
    renderWithProviders(<Reader slot={vi.fn<RenderReplySlot>(() => null)} />);
    await screen.findByRole("heading", { name: Q4.subject });
    const note = await screen.findByRole("region", { name: "Where things stand" });
    expect(within(note).queryByRole("button", { name: "Reply" })).toBeNull();
  });

  it("says an ask that lapsed has expired, and offers nothing to resolve", async () => {
    // A sign-in link valid for ten minutes, long past (campbooks-cloud#275).
    // Read already: the read call's answer would replace it.
    routes({
      ...Q4,
      unread_count: 0,
      need: null,
      lapsed_need: {
        reason: "do",
        question: "asks_action",
        message_id: "0192f1a0-0000-7000-8000-0000000000e1",
        lapsed_at: "2026-10-11T11:10:00Z",
      },
    });
    renderWithProviders(<Reader />);
    await screen.findByRole("heading", { name: Q4.subject });
    expect(await screen.findByText("Expired")).toBeVisible();
    expect(screen.queryByRole("button", { name: /^Not needed/u })).toBeNull();
    expect(screen.queryByRole("button", { name: /^Yes/u })).toBeNull();
  });

  it("deletes with #, reports spam with !, and is done with E", async () => {
    routes();
    const onMove = vi.fn<OnMove>();
    renderWithProviders(<Reader onMove={onMove} />);
    await screen.findByRole("heading", { name: Q4.subject });
    expect(await screen.findByRole("button", { name: /^Delete/u })).toBeVisible();
    expect(screen.getByRole("button", { name: /^Report spam/u })).toBeVisible();
    for (const key of ["#", "!", "e"]) {
      fireEvent.keyDown(document.body, { key });
    }
    expect(onMove.mock.calls.map(([, move]) => move)).toEqual(["trash", "spam", "archive"]);
  });

  it("in Spam: says so, offers Not spam on !, and no reply, Scout or images", async () => {
    // Read already: the read call's answer would replace it.
    routes({ ...Q4, unread_count: 0, archived: true, trashed: true, spam: true, need: null });
    const onMove = vi.fn<OnMove>();
    const slot = vi.fn<RenderReplySlot>(() => null);
    renderWithProviders(<Reader slot={slot} onMove={onMove} />);
    expect(await screen.findByText("This conversation is in Spam.")).toBeVisible();
    expect(await screen.findByText(/2 remote images hidden/u)).toBeVisible();
    expect(screen.queryByRole("button", { name: /^Show images/u })).toBeNull();
    expect(screen.queryByRole("button", { name: /^Reply/u })).toBeNull();
    expect(screen.queryByRole("button", { name: /^Report spam/u })).toBeNull();
    expect(slot).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: /^Not spam/u }));
    fireEvent.keyDown(document.body, { key: "!" });
    expect(onMove.mock.calls.map(([, move]) => move)).toEqual(["not_spam", "not_spam"]);
  });

  it("in Trash: says so, and Restore (#) brings it back where it was", async () => {
    routes({ ...Q4, unread_count: 0, trashed: true, spam: false, need: null });
    const onMove = vi.fn<OnMove>();
    renderWithProviders(<Reader onMove={onMove} />);
    expect(await screen.findByText("This conversation is in Trash.")).toBeVisible();
    expect(screen.queryByRole("button", { name: /^Delete/u })).toBeNull();
    fireEvent.keyDown(document.body, { key: "#" });
    // E does nothing here: Restore is the way back.
    fireEvent.keyDown(document.body, { key: "e" });
    expect(onMove.mock.calls.map(([, move]) => move)).toEqual(["untrash"]);
  });
});
