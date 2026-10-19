## MODIFIED Requirements

### Requirement: Reading conversations
`GET /api/v1/w/{workspace_id}/inboxes` (`conversations.list_inboxes`) SHALL
list the caller's inboxes with their role and unread count.
`GET …/conversations` (`conversations.list`) SHALL list the caller's
conversations by stream and inbox, in the inbox or archived, with an opaque
cursor and a limit of 1 to 100 (else 400). Each listed conversation SHALL say
whether its newest message (the one `latest_from` and `snippet` come from)
is one the inbox sent (`latest_from_you`). In the inbox, pinned (starred)
conversations SHALL come first, then the rest, each newest first, and a
cursor SHALL page across the two without losing or repeating one; archived
lists and lists by `ids` SHALL be newest first. With `sent` true the list SHALL
instead hold the conversations in which a message was sent from the inbox
(`last_sent_at` set), in any stream and whether archived or not, trash and spam
left out, the caller's newest sent message first and nothing pinned first;
`stream` and `inbox` narrow it and `archived` is ignored. A cursor from a list
of the other kind SHALL be refused with `invalid_cursor`. Each listed
conversation SHALL carry `last_sent_at` (the time of that newest sent message)
and `sent_to` (its first To recipient, else its first Cc), both absent when no
message was sent. A cursor given before pins
existed SHALL read as one among the unpinned conversations. Pinning SHALL
stay the provider's star (Gmail's STARRED), synced as before.
`GET …/conversations/{conversation_id}` (`conversations.get`) SHALL return the
conversation, up to its 100 newest messages with their plain-text bodies, and
the decide pass's answers about its newest inbound message.
`GET …/messages/{message_id}/body` (`conversations.get_message_body`) SHALL
return the text and the sanitised HTML bodies as JSON, with the HTML body's
filtered `@media` rules in `css`; HTML SHALL never be served as `text/html`.

#### Scenario: Paging
- **WHEN** a member lists three conversations two at a time
- **THEN** the first page carries a `next_cursor` and the second page holds the oldest conversation and none

#### Scenario: A malformed cursor
- **WHEN** the cursor is not one the API gave
- **THEN** the response is 400 with code `invalid_cursor`

#### Scenario: A pinned conversation stays on top
- **WHEN** a member pins the oldest of three conversations and lists them one at a time
- **THEN** the pinned one comes first, then the other two newest first, each exactly once

#### Scenario: Who wrote last
- **WHEN** the newest message of a conversation is one Sam sent from the inbox
- **THEN** it is listed with `latest_from_you` true, and a conversation whose newest message came in is listed with it false

#### Scenario: A conversation you started
- **WHEN** Sam started a conversation (no Inbox label, so it is archived) and lists with `sent=true`
- **THEN** it is listed with `archived` true, `last_sent_at` and `sent_to` set, and it is not in the inbox list

#### Scenario: Sent is ordered by your latest message
- **WHEN** someone replies later to an older conversation Sam started
- **THEN** in the sent list it keeps its place by `last_sent_at`, not by `last_message_at`, and a cursor from the sent list is refused by the others


## ADDED Requirements

### Requirement: The time of your latest message
`conversations.conversations.last_sent_at` SHALL be the `sent_at` of the
conversation's newest outbound message that is not in the trash or spam, or
null when there is none. It SHALL be recomputed with the conversation's other
counts and flags whenever they are (ingest of a change batch, the provider
reporting a sent message, a message removed or moved between conversations,
any member action), and a migration SHALL backfill it for existing
conversations. A conversation Sam started SHALL stay out of the inbox until a
message arrives in it with the inbox label.

#### Scenario: A conversation you started
- **WHEN** a batch holds only a message Sam sent, without the Inbox label
- **THEN** its conversation is archived, has `last_sent_at` equal to that message's `sent_at`, and is listed by the sent list only

#### Scenario: They replied
- **WHEN** a message with the Inbox label arrives in that thread
- **THEN** the conversation is in the inbox again, its `last_sent_at` is unchanged, and the sent list shows its newest message is not Sam's

#### Scenario: Trashed
- **WHEN** Sam trashes the conversation, or every message he sent in it
- **THEN** it leaves the sent list, and coming back from the trash puts it back
