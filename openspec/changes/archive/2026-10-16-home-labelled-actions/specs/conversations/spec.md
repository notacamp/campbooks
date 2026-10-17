## MODIFIED Requirements

### Requirement: Reading conversations
`GET /api/v1/w/{workspace_id}/inboxes` (`conversations.list_inboxes`) SHALL
list the caller's inboxes with their role and unread count.
`GET …/conversations` (`conversations.list`) SHALL list the caller's
conversations by stream and inbox, in the inbox or archived, with an opaque
cursor and a limit of 1 to 100 (else 400). In the inbox, pinned (starred)
conversations SHALL come first, then the rest, each newest first, and a
cursor SHALL page across the two without losing or repeating one; archived
lists and lists by `ids` SHALL be newest first. A cursor given before pins
existed SHALL read as one among the unpinned conversations. Pinning SHALL
stay the provider's star (Gmail's STARRED), synced as before.
`GET …/conversations/{conversation_id}` (`conversations.get`) SHALL return the
conversation, up to its 100 newest messages with their plain-text bodies, and
the decide pass's answers about its newest inbound message.
`GET …/messages/{message_id}/body` (`conversations.get_message_body`) SHALL
return the text and the sanitised HTML bodies as JSON; HTML SHALL never be
served as `text/html`.

#### Scenario: Paging
- **WHEN** a member lists three conversations two at a time
- **THEN** the first page carries a `next_cursor` and the second page holds the oldest conversation and none

#### Scenario: A malformed cursor
- **WHEN** the cursor is not one the API gave
- **THEN** the response is 400 with code `invalid_cursor`

#### Scenario: A pinned conversation stays on top
- **WHEN** a member pins the oldest of three conversations and lists them one at a time
- **THEN** the pinned one comes first, then the other two newest first, each exactly once
