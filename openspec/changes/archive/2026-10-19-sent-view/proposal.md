# Proposal

## Why

A conversation you start has no Inbox label, so it is archived on arrival and
shows nowhere until someone replies. You cannot see mail you sent. The
approved Sent view (campbooks-cloud#240) lists every conversation you have
written in.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/240

## What Changes

- **`conversations` module**
  - `conversations.conversations.last_sent_at`: the `sent_at` of the newest
    message you sent in it, kept with the other counts and flags, backfilled
    by the migration, with a partial index per inbox for the Sent list.
  - `conversations.list` takes `sent` (default false): conversations with a
    sent message, in any stream, archived or not, trash and spam left out,
    your latest message first, no pins first. `stream` and `inbox` narrow it.
    Its cursor carries `(last_sent_at, id)` and is refused by other lists.
  - `ConversationSummary` gains `last_sent_at` and `sent_to` (the first
    recipient of that newest sent message).
- **Client**: a "Sent" item under the four streams in the sidebar (G then 5),
  a "Sent" chip on phones, a `/w/:workspaceId/inbox/sent` list that reuses the
  inbox screen and reader. Rows lead with "To <name>", show the time of your
  latest message and a "Replied" chip when the newest message is not yours.
  An empty Sent says so.
- Demo data and a `sent` recording scenario.

## Non-goals

- Drafts and scheduled sends have their own places; Sent lists conversations
  with a sent message, not outbox items.
- No search inside Sent, no per-recipient grouping, no unread count.
- No change to how a started conversation is archived on arrival, or to
  how a reply brings it back into the inbox.

## Impact

Modules: `conversations` (server and client). No new module, no new
dependency, no new event, no new scope (`conversations:read`).
