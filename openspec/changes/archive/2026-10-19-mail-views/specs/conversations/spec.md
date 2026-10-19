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
archived or not; `archive`, those with no message in the inbox and not
binned; `spam`, those binned as spam; and `trash`, those binned in the
trash. In the inbox, pinned (starred) conversations SHALL come first, then
the rest, each newest first, and a cursor SHALL page across the two without
losing or repeating one. Sent SHALL be ordered by `last_sent_at`, newest
first, its cursor keyed on it; archive, spam, trash and lists by `ids` SHALL
be newest first and never carry a pin in their cursor. Each view SHALL page
over an index of its own order. `archived=true` is deprecated: it SHALL list
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

#### Scenario: The deprecated archived
- **WHEN** a client asks for `archived=true`, then for `archived=true&view=sent`
- **THEN** the first lists what `view=archive` lists, and the second is 400 `invalid_view`

#### Scenario: Other inboxes
- **WHEN** a member of the workspace who is not in an inbox lists any view
- **THEN** none of that inbox's conversations is listed

#### Scenario: Who wrote last
- **WHEN** the newest message of a conversation is one Sam sent from the inbox
- **THEN** it is listed with `latest_from_you` true, and a conversation whose newest message came in is listed with it false

### Requirement: Trash and spam
`POST …/conversations/{conversation_id}/trash` (`conversations.trash`) and
`/untrash` (`conversations.untrash`), and `POST …/conversations/{conversation_id}/spam`
(`conversations.report_spam`) and `/not-spam` (`conversations.not_spam`), SHALL
be undo pairs (`conversations:write`, the catalogue's `write` on the
conversation). Trash SHALL put every message of the conversation in the trash,
leaving where it was otherwise unchanged, and untrash take them out, back
there: in the inbox if it was there, archived if it was archived. Spam SHALL
report every message as spam (in spam, out of the inbox); not spam SHALL take
them out of spam and put the mail the inbox received back in the inbox,
whoever reported it and whether or not it was archived before, and never a
message the member sent. Trash and spam SHALL exclude each other. A
conversation whose every message is in the trash or spam is binned
(`trashed`); it SHALL be binned as spam (`spam`) when one of its messages is
in spam and not in the trash, and else be in the trash, so it is listed in
Spam or in Trash, never both, and in no other view. Each operation SHALL
change only what is not already so, publish
`conversations.conversation_changed` (change `messages`) and
`conversations.changes_requested` with one `trash`, `untrash`, `spam` or
`unspam` item per message that changed (`untrash` and `unspam` with the
message's direction, `inbound`), and work out the conversation's need again:
mail in the trash or spam needs nothing.

#### Scenario: Report and delete
- **WHEN** a member reports a phishing conversation as spam
- **THEN** it leaves every list but Spam, each of its messages gets a `spam` item in `changes_requested`, its Review need is cleared, and not spam brings it back to the inbox

#### Scenario: Spam the provider flagged
- **WHEN** a member takes a conversation the provider put in spam, holding their own reply, out of spam
- **THEN** the received messages are in the inbox, their reply is not, and each `unspam` item says whether its message was received

#### Scenario: Trash after spam
- **WHEN** a member reports a conversation as spam, then trashes it
- **THEN** its messages are in the trash and no longer in spam, and it moves from Spam to Trash

#### Scenario: Restore where it was
- **WHEN** a member deletes one conversation from the inbox and one from the archive, then restores both
- **THEN** the first is in the inbox again and the second in the archive

#### Scenario: Not spam after the archive
- **WHEN** a member archives a newsletter, reports it as spam, then says it is not spam
- **THEN** it is in the inbox, not archived

#### Scenario: A reader
- **WHEN** a reader of the inbox tries to trash a conversation
- **THEN** the response is 403 `inbox_read_only` and nothing is published

#### Scenario: Someone outside the inbox
- **WHEN** a member who cannot see the conversation tries to trash it or report it
- **THEN** the response is 404

## ADDED Requirements

### Requirement: When the inbox last wrote
A conversation SHALL keep `last_sent_at`, the newest `sent_at` of its
outbound messages in neither spam nor the trash (none when there is none),
and `spam`, both worked out from its messages whenever its counts are, and
worked out for every stored conversation when they were introduced.

#### Scenario: A sent message moves to the trash
- **WHEN** the inbox's newest message in a conversation is moved to the trash at the provider
- **THEN** `last_sent_at` falls back to the inbox's message before it, and to none once no message the inbox sent is outside spam and the trash

#### Scenario: Mail stored before
- **WHEN** the migration runs over conversations stored before it
- **THEN** each one's `last_sent_at` and `spam` are what its messages say, and running it again changes nothing

### Requirement: Mail in spam shows no remote images
`conversations.get_message_body` SHALL keep remote images held back for a
message in spam, or in a conversation binned as spam, even when
`show_images=true` is asked for and even when a smart rule trusts its sender:
it SHALL say `images_held: spam`, trust nothing and offer nothing. Once the
conversation is out of spam, images SHALL follow the rules as before.

#### Scenario: A trusted sender reported as spam
- **WHEN** a newsletter whose sender a rule trusts is reported as spam, and its body is asked for with `show_images=true`
- **THEN** no remote image is kept, `images_trusted` and `images_offer` are false and `images_held` is `spam`; after not spam its images show again
