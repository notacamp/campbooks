## ADDED Requirements

### Requirement: Outbound changes to Gmail
The module SHALL consume `conversations.changes_requested` v1 and make each
item's change at Gmail for the stream that feeds the event's inbox: read and
unread through `UNREAD`, star and unstar through `STARRED`, archive and
unarchive through `INBOX`, a provider label by its Gmail label id, and a move
to the Primary, Updates or Newsletters stream through the matching Gmail
category (`CATEGORY_PERSONAL`, `CATEGORY_UPDATES`, `CATEGORY_PROMOTIONS`, the
other categories removed). A change SHALL be recorded durably, with Gmail's
history id at that moment, before it is attempted, and SHALL be made by the
stream's next run before it reads, with `users.messages.modify`, or
`users.messages.batchModify` for more than ten messages with the same change.
Changes recorded before a run SHALL be merged per message and label, the
latest winning. Every recorded change SHALL end `applied`, `superseded`,
`skipped` or `failed` with a stable reason code and no message content. A
label without a provider key, or that Gmail no longer has, and a move to the
Receipts stream, SHALL be recorded as `skipped`. Only `UNREAD`, `STARRED`,
`INBOX`, the categories and user labels SHALL ever be written; a label change
naming a system label SHALL be skipped. A change that would bring a message
Gmail holds in the trash or spam back to the inbox or a category SHALL be
recorded `skipped` (`in_trash`). A change still pending after the batch
retention period SHALL be recorded `failed` (`expired`) before any run takes
it, and never made.

#### Scenario: Archive reaches Gmail
- **WHEN** a member archives a conversation whose message is in Gmail's inbox
- **THEN** the stream runs, Gmail's message loses `INBOX`, and the change is recorded `applied`

#### Scenario: Every intent kind
- **WHEN** members mark read and unread, star and unstar, archive and unarchive, add and remove a Gmail label, and move conversations to Primary, Updates, Newsletters and Receipts
- **THEN** Gmail's labels change as mapped, and the label without a provider key and the move to Receipts are recorded `skipped`

#### Scenario: Undo before the run
- **WHEN** a member archives and unarchives a conversation before the stream runs
- **THEN** no write is made, the archive is recorded `superseded` (`replaced`) and the unarchive `applied`

#### Scenario: A stale change
- **WHEN** a change recorded ten days ago is still pending when the stream runs
- **THEN** it is recorded `failed` (`expired`), no write is made, and the message's Gmail state is emitted again

#### Scenario: In the trash
- **WHEN** a member unarchives a message, or moves one to Primary, that Gmail holds in the trash or spam
- **THEN** no write is made and the change is recorded `skipped` (`in_trash`)

#### Scenario: Many messages, one call
- **WHEN** one intent marks twelve messages read
- **THEN** a single `batchModify` makes the change

### Requirement: Write-backs never echo
Changes that came from Gmail SHALL never be written back to Gmail. After a
write, the adapter's index SHALL hold the labels it set, so that Gmail's
history record of that write produces no change in the next batch. While a
member's change is pending, a run that emits that message SHALL emit it with
the change applied.

#### Scenario: The next sync after a write-back
- **WHEN** a member's star has been written to Gmail and the stream runs again
- **THEN** no `connectors.messages_changed` batch carries that message, and no new outbound change is recorded

#### Scenario: A Gmail change still arrives
- **WHEN** a member's archive is written while the same message was starred in Gmail and not yet read
- **THEN** the next batch carries the message starred and archived

### Requirement: Last writer wins in Gmail's order
Per message and label, the later write in Gmail's history order SHALL win. A
member's change SHALL be placed at the history id stamped when it was
recorded: a Gmail change to the same label after that point SHALL win (the
write is dropped as `superseded`, `changed_at_provider`, and the message's
Gmail state is emitted again), and a Gmail change before it SHALL be
overwritten. A change SHALL be made whole or not at all: when any label it
would write was changed at Gmail later, none of it is written. History
records inside the write window of one of the module's own runs SHALL count
as its own writes; a window the run could not close (Gmail's history id after
the write could not be read) SHALL be closed at the history id the next read
reaches, and SHALL cover nothing once open for ten minutes. Without a stamp, or when Gmail no
longer has the history, a change SHALL apply in arrival order.

#### Scenario: Changed in Gmail after the member
- **WHEN** a member stars a message and, before the write-back, it is starred and unstarred again in Gmail
- **THEN** Gmail stays unstarred, the change is `superseded` (`changed_at_provider`), and the next batch tells conversations the message is not starred

#### Scenario: A move overtaken in part
- **WHEN** a member moves a conversation to Newsletters and, before the write-back, it is moved to Social in Gmail
- **THEN** no category is written, Gmail keeps Social only, and the change is `superseded` (`changed_at_provider`)

#### Scenario: Changed in Gmail before the member
- **WHEN** a message is archived in Gmail, and then a member who has not yet seen it unarchives it in Campbooks
- **THEN** the message is back in Gmail's inbox

#### Scenario: Undo after the write
- **WHEN** a member archives a message, the archive is written, and the member unarchives it while that write was being made
- **THEN** the unarchive is applied, not mistaken for a change made in Gmail

### Requirement: Write-back retries and failures
Provider trouble while writing (429, 5xx, a network failure, a refused
token) SHALL stop the push, keep the unwritten changes pending, and use the
run's backoff, throttling and health rules; writes SHALL take Gmail quota
from the shared provider budget. A change Gmail refuses for good, or that
has met trouble eight times, SHALL be recorded `failed` with a reason code
and no content, and the message's Gmail state SHALL be emitted again. A
message Gmail no longer has SHALL be recorded `skipped` (`message_gone`).

#### Scenario: Throttled, then written
- **WHEN** Gmail answers 429 with `Retry-After` to a write
- **THEN** the change stays pending, the stream is throttled until then, and the next run writes it

#### Scenario: A message that refuses changes
- **WHEN** Gmail refuses the change for one message of a conversation
- **THEN** the other messages are changed, that one is recorded `failed` (`refused`), and conversations is told its Gmail state
