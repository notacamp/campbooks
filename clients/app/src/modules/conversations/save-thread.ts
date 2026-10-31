import type { SaveThread } from "~/modules/pages";

import type { MessageView } from "./api/queries";
import { displayName, splitQuoted } from "./model";

/**
 * A thread as Save to page takes it (campbooks-cloud#364): oldest first,
 * each message with its own words only (the quoted history it carries is
 * dropped, as the reader folds it away) and the files attached to it (not
 * the pictures its body shows). `messages` must hold their words: ask for a
 * `text_unread` one first.
 */
export function saveThreadOf(
  conversationId: string,
  subject: string,
  messages: readonly MessageView[],
): SaveThread {
  return {
    id: conversationId,
    subject,
    messages: messages
      .toSorted((a, b) => a.sent_at.localeCompare(b.sent_at))
      .map((message) => ({
        id: message.id,
        from: displayName(message.from),
        sentAt: message.sent_at,
        text: splitQuoted(message.text?.content ?? "").own,
        attachments: message.attachments
          .filter((file) => !file.in_body && !file.inline)
          .map((file) => ({ id: file.id, name: file.filename, size: file.size })),
      })),
  };
}
