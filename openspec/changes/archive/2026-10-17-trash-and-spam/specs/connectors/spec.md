## ADDED Requirements

### Requirement: Trash and spam at the provider
The outbound changes SHALL accept the kinds `trash`, `untrash`, `spam` and
`unspam`, with the merge, supersede, retry and give-up rules of the other
kinds; trash and spam SHALL exclude each other (a change into one takes the
message out of the other). In Gmail, trash SHALL be made with
`users.messages.trash` and untrash with `users.messages.untrash` (Gmail puts
back the labels the message had); spam SHALL add `SPAM` and remove `INBOX`,
remembering whether the message was in the inbox, and unspam SHALL remove
`SPAM` and add `INBOX` only if it was. In Zoho, trash SHALL move the message
to the account's Trash folder and spam SHALL use Zoho's report
(`moveToSpam`), both remembering the folder the message was in and whether it
was archived there; untrash and unspam SHALL move it back to that folder,
archived again if it was (Zoho's own "not spam" mode is documented
inconsistently and is not used, so taking a message out of spam does not
tell Zoho's filter). A Trash folder the account's listing did not show, or a
message with no remembered place, SHALL never be guessed: that part of the
change SHALL be recorded `skipped` (`not_movable`) and the provider's state
emitted again. A change taking a message out of the trash or spam SHALL not
be skipped as `in_trash`. After a write, the index SHALL hold the state
written, so the next run emits nothing for it.

#### Scenario: Report in Gmail
- **WHEN** a member reports a conversation whose message is in Gmail's inbox, then takes it out of spam
- **THEN** the message gains `SPAM` and loses `INBOX`, then loses `SPAM` and is back in the inbox; a message reported while archived goes back to the archive

#### Scenario: Trash and back in Zoho
- **WHEN** a member trashes a message filed in one of their own Zoho folders, then untrashes it
- **THEN** Zoho receives `moveMessage` to the Trash folder, then back to that folder, both changes are `applied`, and neither write comes back as a change

#### Scenario: An unknown Trash folder
- **WHEN** a Zoho account's folder listing shows no Trash folder and a member trashes a message there
- **THEN** no move is made and the change is recorded `skipped` (`not_movable`)
