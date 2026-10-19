import type { ReactNode } from "react";

import type { Schema } from "~/lib/api";

/**
 * The reader's slot for answering a conversation (the send slice fills it).
 *
 * The reader owns the Reply / Reply all / Forward buttons and their
 * shortcuts (`item.reply` R, `item.reply_all` Shift+R, `item.forward` F); a press
 * sets {@link ReplySlotContext.request}. The slot renders the composer at the
 * end of the thread, and calls `onClose` when it is done (sent, discarded or
 * closed), which sets `request` back to `null`. With no renderer, the reader
 * hides the buttons.
 */
export type ReplyMode = "reply" | "reply_all" | "forward";

/** One press of Reply, Reply all or Forward. */
export interface ReplyRequest {
  mode: ReplyMode;
  /** The message answered: the newest one, unless a message's own button was used. */
  messageId: string;
  /** Changes on every press, so pressing the same button again re-opens or re-focuses. */
  nonce: number;
}

export interface ReplySlotContext {
  workspaceId: string;
  inboxId: string;
  conversation: Schema<"ConversationSummary">;
  /** Oldest first, as `conversations.get` returns them. */
  messages: Schema<"MessageView">[];
  /** `null` until a button is pressed (render nothing, or a collapsed box). */
  request: ReplyRequest | null;
  /** A saved draft to open on arrival (Drafts), whatever its kind. */
  draftId?: string | undefined;
  onClose: () => void;
}

export type RenderReplySlot = (context: ReplySlotContext) => ReactNode;
