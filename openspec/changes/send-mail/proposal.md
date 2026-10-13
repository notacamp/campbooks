# Proposal

## Why

Campbooks Next reads and tidies mail but cannot answer it. A working inbox
before the production cutover (campbooks-cloud#94) needs reply, reply all,
forward and a new message, sent through the member's own mailbox (Gmail
first), safe against double sends and with a moment to take a send back.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/101.

Modules touched: `conversations` (drafts, sending, the composer UI) and
`connectors` (the Gmail send adapter). No new module and no new cross-module
dependency: the two talk through two new events and one new blob document,
each with a contract in `contracts/` (ADR 0010, ADR 0012).

## What Changes

- **Drafts in `conversations`** (`conversations.drafts`): a member's message
  being written, private to its author, for a new message or a reply, reply
  all or forward of a message. Creating a reply or forward fills the
  recipients, subject, `In-Reply-To`/`References` and the provider thread on
  the server; the client adds the quoted history. Saved as the member types
  (autosave with a version check). A forward carries the original
  attachments, which the member may drop.
- **Operations** (all `conversations:send`, a new scope; the `sender` or
  `manager` role on the inbox for users):
  `conversations.list_drafts`, `.create_draft`, `.get_draft`,
  `.update_draft`, `.delete_draft`, `.send_draft` (high risk: the assistant
  must ask), `.cancel_send` (its undo).
- **Undo send.** Sending schedules the draft; the job
  `conversations.release_send` runs after `CAMPBOOKS_CONVERSATIONS__UNDO_SEND_SECONDS`
  (default 10, 0 to 60). Until then `cancel_send` puts it back to a draft.
- **Outbound intent.** On release, `conversations` writes an
  **outgoing message** document (blob, `contracts/outgoing-message.v1.schema.json`)
  and publishes `conversations.send_requested` v1 (ids and the blob only).
- **Gmail send in `connectors`.** The consumer `connectors.queue_send`
  records the send (`connectors.sends`, keyed by the send id) and enqueues
  `connectors.send_message`, which builds RFC 5322 (`mail-builder`) and
  calls `users.messages.send` with the thread id. **Idempotent:** an
  attempt is recorded before the call; a retry after an unclear failure
  (timeout, 5xx) first looks the message up by its `Message-ID` and only
  sends if Gmail does not have it.
- **The sent message comes back at once.** On success the adapter indexes
  the message it sent and announces it in a `MessageChangeBatch` exactly as
  sync would, so it appears in the conversation without waiting for the next
  poll, and the later sync recognises it (one message, never two).
  `connectors.send_completed` v1 tells `conversations` the outcome;
  `conversations` then deletes the draft (`conversations.draft_sent`) or
  marks it failed with a reason (`conversations.send_failed`).
- **Composer UI** (`clients/app`, module `conversations`): the reader's
  reply slot (#100) opens a composer for Reply / Reply all / Forward; a New
  message screen; From is the inbox; To/Cc/Bcc as chips; quoted history;
  plain text with basic formatting (bold, italic, lists, links), rendered to
  a safe HTML alternative on the server; Undo in a toast. Shortcuts: `c`
  new message, `⌘↵` send, `Esc` close, `⌘B`/`⌘I` formatting, all shown.
  The reader owns `r` / `a` / `f`. Four locales, both themes, 375px and
  desktop, Storybook stories.

## Non-goals

- Uploading new attachments (forwarding existing ones only); follow-up.
- Scheduled send at a chosen time, signatures, rewrites and AI drafts
  (`compose.draft` stays as it is).
- Mirroring drafts to Gmail's Drafts folder (Campbooks keeps its own; Gmail
  drafts stay skipped by sync).
- Sending from Microsoft, Zoho or IMAP (the outgoing document is
  provider-neutral; their adapters come with those providers).
- A rich-text (WYSIWYG) editor.
- Outbound mirroring of read/star/archive/label changes
  (`conversations.changes_requested`), which is its own change.
