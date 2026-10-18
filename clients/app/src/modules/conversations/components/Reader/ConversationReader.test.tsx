// A reader without the send slice's composer offers no Reply: none that
// would do nothing (campbooks-cloud#220).
import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { INBOX, Q4, Q4_DETAIL, W } from "../../../../../test/demo/conversations";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../../test/render";
import { useInboxActions } from "../InboxScreen/use-inbox-actions";

import { ConversationReader } from "./ConversationReader";
import type { RenderReplySlot } from "./ReplySlot";

function Reader({ slot }: { slot?: RenderReplySlot }) {
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
      onDone={vi.fn()}
      renderReplySlot={slot}
    />
  );
}

function routes() {
  return mockApi({
    [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
    [`GET ${W}/conversations/${Q4.id}`]: { body: Q4_DETAIL },
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
});
