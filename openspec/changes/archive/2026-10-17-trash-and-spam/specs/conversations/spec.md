## ADDED Requirements

### Requirement: Trash and spam
`POST …/conversations/{conversation_id}/trash` (`conversations.trash`) and
`/untrash` (`conversations.untrash`), and `POST …/conversations/{conversation_id}/spam`
(`conversations.report_spam`) and `/not-spam` (`conversations.not_spam`), SHALL
be undo pairs (`conversations:write`, the catalogue's `write` on the
conversation). Trash SHALL put every message of the conversation in the trash,
leaving where it was otherwise unchanged, and untrash take them out, back
there. Spam SHALL report every message as spam (in spam, out of the inbox),
remembering per message whether it was in the inbox; not spam SHALL take them
out of spam and back in the inbox only those that were there before, never a
message the member sent. Trash and spam SHALL exclude each other. Each SHALL
change only what is not already so, publish
`conversations.conversation_changed` (change `messages`) and
`conversations.changes_requested` with one `trash`, `untrash`, `spam` or
`unspam` item per message that changed, and work out the conversation's need
again: mail in the trash or spam needs nothing.

#### Scenario: Report and delete
- **WHEN** a member reports a phishing conversation as spam
- **THEN** it leaves every list, each of its messages gets a `spam` item in `changes_requested`, its Review need is cleared, and not spam brings it back to the inbox

#### Scenario: Reported while archived
- **WHEN** a member archives a conversation, reports it as spam, then takes it out of spam
- **THEN** it is archived again, not in the inbox

#### Scenario: Trash after spam
- **WHEN** a member reports a conversation as spam, then trashes it
- **THEN** its messages are in the trash and no longer in spam

#### Scenario: A reader
- **WHEN** a reader of the inbox tries to trash a conversation
- **THEN** the response is 403 `inbox_read_only` and nothing is published

#### Scenario: Someone outside the inbox
- **WHEN** a member who cannot see the conversation tries to trash it or report it
- **THEN** the response is 404
