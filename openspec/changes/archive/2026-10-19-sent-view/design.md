# Design

## Data

`conversations.conversations.last_sent_at timestamptz NULL` (schema
`conversations`, no cross-schema reference). It is the `max(sent_at)` of the
conversation's messages with `direction = 'outbound'` that are not in the
trash or spam. Migration `conversations__last_sent_at` adds the column and
backfills it from `conversations.messages`; migration
`conversations__sent_list_index` builds, concurrently and as a migration of its
own like the other list indexes, `(inbox_id, last_sent_at DESC, id DESC)
WHERE last_sent_at IS NOT NULL AND NOT trashed`.

## Where `last_sent_at` is maintained

Every site that maintains `last_message_at` or changes what a conversation
holds was checked.

- `domain::conversation::recount` computes the counts and flags from the
  messages and now also `last_sent_at`. It is the only writer of the column
  after the backfill. Everything that changes a conversation's messages ends
  in it: ingest (`apply_changes::finish` recounts each touched or new
  conversation), a message the provider removes (the deleted message's
  conversation is touched), merging conversations by threading (the emptied
  one is recounted and deleted, the target is touched), and every member
  action (trash, untrash, spam, not spam, archive, star, read: they publish
  through `Changed::publish`, which recounts).
- The insert of a new conversation in `apply_changes` sets `last_message_at`
  from the first message and leaves `last_sent_at` NULL; the recount that
  follows in the same transaction sets it. No separate outbound branch is
  needed.
- `record_send` and `send_draft` do not insert a message. The sent message
  comes back from the provider in the next change batch and is ingested, so
  `last_sent_at` is set by that recount. Until then the conversation shows
  nowhere in Sent, as it shows nowhere in the thread.
- `purge_workspace`, `forget_*`, `backfill_needs`, `rule_notices` and
  `search_source` read or delete whole rows and need nothing.
- A trashed or spammed message of yours stops counting: its conversation's
  `last_sent_at` falls back to an earlier message of yours, or NULL. A
  conversation whose messages are all trashed or spam has `trashed` true and
  is out of Sent either way.

## The list

`conversations.list` with `sent=true` is a third branch of `fetch_page`:
`last_sent_at IS NOT NULL AND NOT trashed`, ordered by
`(last_sent_at, id)` descending, `stream` narrowing as before, `archived`
ignored. It still goes through the same visibility check (only inboxes the
caller can read), so another member's inbox stays out.

### Cursor

Cursors are `pinned|at|id` (and the older `at|id`). A Sent cursor is
`s|at|id`, with `at` the `last_sent_at`. `decode_cursor` reads the kind, and
the handler refuses (400 `invalid_cursor`) a cursor whose kind does not match
the list asked for. The kind prefix keeps a Sent cursor from silently paging a
stream list by the wrong timestamp, and the reverse.

### `sent_to`

A lateral subquery reads the `recipients` of the newest outbound, not trashed
message, only for rows with `last_sent_at`. `sent_to` is its first To
recipient, else its first Cc. The domain `Address` type is reused.

### Replied

"Someone answered since" is `last_message_at > last_sent_at` with an inbound
newest message, which is `!latest_from_you` for a row that has a
`last_sent_at`. No new field is added; the client marks the row.

## Mail you start

A conversation you start has no Inbox label, so its messages have
`in_inbox = false` and the recount archives it. A reply that arrives with the
Inbox label sets `in_inbox`, and the recount un-archives it. A test pins this.

## Client

`ListView = Stream | "sent"`. The sent list has its own query key
(`conversations.list` with `sent: "true"`), so no stream list ever holds a
Sent row and the cache placement (`placeInLists`) only updates a row in place
in Sent, never adds or removes one. The change feed already invalidates every
`conversations.list` query on a conversation change, so a message you send
shows in Sent as soon as the provider sync ingests it. Static route segments
win over `$stream`, so `/inbox/sent` and `/inbox/sent/$conversationId` sit
beside the stream routes. The sidebar item uses a new `apart` variant of
`NavItem` (a gap above it) to set it slightly apart.

## Scopes, events, AI

No new scope: the list needs `conversations:read` as before. No event is
added. No AI is used.
