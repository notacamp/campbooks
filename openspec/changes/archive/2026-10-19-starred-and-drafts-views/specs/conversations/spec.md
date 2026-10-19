## MODIFIED Requirements

### Requirement: Reading conversations
`GET /api/v1/w/{workspace_id}/inboxes` (`conversations.list_inboxes`) SHALL
list the caller's inboxes with their role and unread count.
`GET …/conversations` (`conversations.list`) SHALL list the caller's
conversations by inbox, in one view (`view`), by stream when `stream` is
given, with an opaque cursor and a limit of 1 to 100 (else 400). The views
SHALL be: `inbox` (the default), the conversations with a message in the
inbox and none binned; `sent`, every conversation the inbox wrote in outside
spam and the trash (`last_sent_at` set, not `trashed`), in any stream,
archived or not; `starred`, those pinned (`starred`) and not binned, in any
stream, archived or not; `archive`, those with no message in the inbox and not
binned; `spam`, those binned as spam; and `trash`, those binned in the
trash. In the inbox, pinned (starred) conversations SHALL come first, then
the rest, each newest first, and a cursor SHALL page across the two without
losing or repeating one. Sent SHALL be ordered by `last_sent_at`, newest
first, its cursor keyed on it; starred, archive, spam, trash and lists by `ids`
SHALL be newest first and never carry a pin in their cursor. Sent, archive,
spam and trash SHALL page over an index of their own order; starred SHALL
page over the list index, since few conversations are pinned. `archived=true` is deprecated: it SHALL list
the archive as `view=archive` does, and with any other `view` the response
SHALL be 400 `invalid_view`. Lists by `ids` SHALL ignore the view and the
stream. A cursor given before pins existed SHALL read as one among the
unpinned conversations. Pinning SHALL stay the provider's star (Gmail's
STARRED), synced as before. Each listed conversation SHALL say whether its
newest message (the one `latest_from` and `snippet` come from) is one the
inbox sent (`latest_from_you`). A summary SHALL say whether the conversation is
`archived`, `trashed` and binned as `spam`, when the inbox last wrote in it
(`last_sent_at`), whom that message went to (`sent_to`: to, then cc) and
whether someone wrote back since (`replied_since_sent`).
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

#### Scenario: Sent
- **WHEN** the inbox started one conversation (none of it in the inbox), replied in another and only received a third
- **THEN** `view=sent` lists the first two, the one written in last first, and the inbox lists neither the first nor any binned one

#### Scenario: Paging Sent
- **WHEN** a member lists three sent conversations two at a time
- **THEN** the second page holds the one the inbox wrote in longest ago, and no cursor

#### Scenario: Starred
- **WHEN** a member pins one conversation in the inbox, pins and archives another, and pins a third that is then binned
- **THEN** `view=starred` lists the first two, the more recent first, and not the binned one, and unpinning one takes it out

#### Scenario: Paging Starred
- **WHEN** a member lists three pinned conversations two at a time
- **THEN** the second page holds the oldest of them and no cursor

#### Scenario: The deprecated archived
- **WHEN** a client asks for `archived=true`, then for `archived=true&view=sent`
- **THEN** the first lists what `view=archive` lists, and the second is 400 `invalid_view`

#### Scenario: Other inboxes
- **WHEN** a member of the workspace who is not in an inbox lists any view
- **THEN** none of that inbox's conversations is listed

#### Scenario: Who wrote last
- **WHEN** the newest message of a conversation is one Sam sent from the inbox
- **THEN** it is listed with `latest_from_you` true, and a conversation whose newest message came in is listed with it false
