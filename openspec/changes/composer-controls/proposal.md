## Why

Inside the composer you can't change who a message is from (except on a new
message, and only from a separate dropdown above it). You can't switch
between Reply, Reply all and Forward without starting over, you can't see a
reply's subject, and Scout can help only in the Reply dialog, not while you
write. This is phase 5 of the composer rework, from the approved proposal
(https://claude.ai/artifact/BxLcKwjxF6gHbpfiWCeW8M).

Tracking: https://github.com/notacamp/campbooks-cloud/issues/394 (epic #389).

Modules touched:

- **`conversations` (server):** `update_draft` gains `inbox_id`, which moves
  a draft to another inbox the member may send from.
- **Client:** the composer's From picker, the mode switch, the subject on
  replies, and a Scout menu.

## What Changes

- **From inside the composer.** It shows when the member may send from more
  than one inbox, for every kind of draft, and replaces the separate
  dropdown on New message.
  - The draft moves to that inbox, and its signature switches to the
    member's signature there (once signatures ship).
  - A reply moved to another inbox keeps its `In-Reply-To` and
    `References`, so the recipient's mail still threads it. It loses the
    provider thread key, because that belongs to the first mailbox.
- **Reply ▾ in the composer's title** switches between Reply, Reply all and
  Forward, keeping the text written so far. It refills To, Cc, the subject,
  the quote and the forwarded attachments for the new kind.
- **The subject** is shown and editable on every kind, not only new
  messages and forwards.
- **Scout ▾ (⌘J)** offers Shorter, Warmer, Fix spelling and Change…, on
  reply-kind drafts (reply, reply all, nudge) when Scout can write replies
  in the workspace.
  - Each one rewrites the text in place and shows Undo.
  - Scout's text goes in as plain text and is never read as marks or HTML,
    so it can carry no link whose words hide where it goes.

## Non-goals

- Scout rewrites for new messages and forwards, which have no message to
  answer. They'd need a new prompt and an A/B check.
- Aliases (send-as) inside one mailbox.
- Keeping a minimised draft while you read other mail.

## Impact

- **Server:**
  - `update_draft` gains `inbox_id`, with the role check on the target
    inbox and 403 `inbox_read_only` / 404 as for creating;
  - it clears `thread_key` on a move;
  - the content hash already covers the inbox address;
  - OpenAPI and client types regenerated.
- **Client:**
  - `MessageComposer` gets a From picker, a mode menu and a Scout menu;
  - `DraftComposer` / `ReplyComposer` gain the mode switch, done as a new
    draft of the new kind with the text carried over, then the old draft
    deleted;
  - `NewMessage` drops its separate dropdown;
  - the existing reply stream (`streamReply`) drives the rewrites;
  - i18n.
