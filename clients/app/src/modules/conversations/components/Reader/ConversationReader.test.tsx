// A reader without the send slice's composer offers no Reply: none that
// would do nothing (campbooks-cloud#220).
import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

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
