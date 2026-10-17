## ADDED Requirements

### Requirement: Trash and spam at the provider
The outbound changes SHALL accept the kinds `trash`, `untrash`, `spam` and
`unspam`, with the merge, supersede, retry and give-up rules of the other
kinds. In Gmail, trash SHALL add `TRASH` and untrash remove it; spam SHALL add
`SPAM` and remove `INBOX`, and unspam do the reverse. In Zoho, trash and spam
SHALL move the message to the account's Trash or Spam folder, and untrash and
unspam move it back to the Inbox folder only when it is in the Trash or Spam
folder; a folder the account's listing did not show SHALL never be guessed,
and that part of the change SHALL be recorded `skipped` (`not_movable`). A
change taking a message out of the trash or spam SHALL not be skipped as
`in_trash`. After a write, the index SHALL hold the state written, so the next
run emits nothing for it.

#### Scenario: Report in Gmail
- **WHEN** a member reports a conversation whose message is in Gmail's inbox
- **THEN** the stream runs, the message gains `SPAM` and loses `INBOX`, and the change is `applied`

#### Scenario: Trash and back in Zoho
- **WHEN** a member trashes a conversation in a Zoho mailbox, then untrashes it
- **THEN** Zoho receives `moveMessage` to the Trash folder, then to the Inbox folder, both changes are `applied`, and neither write comes back as a change

#### Scenario: An unknown Trash folder
- **WHEN** a Zoho account's folder listing shows no Trash folder and a member trashes a message there
- **THEN** no move is made and the change is recorded `skipped` (`not_movable`)
