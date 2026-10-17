## ADDED Requirements

### Requirement: Trash and spam at the provider
The outbound changes SHALL accept the kinds `trash`, `untrash`, `spam` and
`unspam`, with the merge, supersede, retry and give-up rules of the other
kinds; trash and spam SHALL exclude each other (a change into one takes the
message out of the other). In Gmail, trash SHALL be made with
`users.messages.trash` and untrash with `users.messages.untrash` (Gmail puts
back the labels the message had); spam SHALL add `SPAM` and remove `INBOX`,
and unspam SHALL remove `SPAM` and add `INBOX` for mail the inbox received
only; nothing else adds `INBOX`. In Zoho, spam SHALL use Zoho's report
(`moveToSpam`) and trash SHALL move the message to the account's Trash folder,
remembering the folder it was in; untrash SHALL move it back there only when
that trash is the latest trash or spam change made to the message, no sync
has seen it leave the trash since, the account's folder listing still shows
that folder and it is neither the Trash nor the Spam folder; otherwise, and
for unspam, received mail SHALL go to the Inbox folder and any other mail
SHALL be recorded `skipped` (`not_movable`), never `message_gone`. Zoho's own
"not spam" mode is not used (it is documented under two names), so not spam
at Zoho does not tell Zoho's filter. A Trash folder the listing did not show
SHALL never be guessed: that change SHALL be recorded `skipped`
(`not_movable`). A change taking a message out of the trash or spam SHALL not
be skipped as `in_trash`. After a write, the index SHALL hold the state
written, so the next run emits nothing for it. Gmail's untrash and Zoho's
`moveToSpam` are as their documentation says and still need a check on a live
account.

#### Scenario: Report in Gmail
- **WHEN** a member reports a conversation whose message is in Gmail's inbox, then takes it out of spam
- **THEN** the message gains `SPAM` and loses `INBOX`, then loses `SPAM` and is back in the inbox; the member's own sent mail only loses `SPAM`

#### Scenario: Report on trashed mail in Gmail
- **WHEN** a member reports one message that is not in Gmail's trash and one that is
- **THEN** the first gets a label change only, with no `messages.untrash`; the second is untrashed, then gains `SPAM`

#### Scenario: Trash and back in Zoho
- **WHEN** a member trashes a message filed in one of their own Zoho folders, then untrashes it
- **THEN** Zoho receives `moveMessage` to the Trash folder, then back to that folder, both changes are `applied`, and neither write comes back as a change

#### Scenario: Trashed from spam
- **WHEN** a member trashes a Zoho message that sits in Spam, then untrashes it
- **THEN** it goes to the Inbox folder, not back to Spam

#### Scenario: Moved at Zoho in between
- **WHEN** a message Campbooks trashed is moved out of the trash at Zoho and back in, and the member untrashes it
- **THEN** the folder remembered from the first trash is not used, and the received message goes to the Inbox folder

#### Scenario: An unknown Trash folder
- **WHEN** a Zoho account's folder listing shows no Trash folder and a member trashes a message there
- **THEN** no move is made and the change is recorded `skipped` (`not_movable`)

#### Scenario: An undo Zoho cannot make
- **WHEN** a member untrashes, or takes out of spam, their own sent mail at Zoho, or a message with no remembered folder that is not received mail
- **THEN** no move is made and the change is recorded `skipped` (`not_movable`); the conversation shows out of the trash or spam at once, and the run that skipped the change emits the message's Zoho state again, so it shows in the trash or spam again
