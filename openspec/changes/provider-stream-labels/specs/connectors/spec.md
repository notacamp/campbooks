## ADDED Requirements

### Requirement: Stream marks at the provider
A stream with no native place at a provider SHALL be carried by a provider
label named `Campbooks/<Stream>` (a stream mark): at Gmail `Campbooks/Receipts`;
at Zoho `Campbooks/Updates`, `Campbooks/Receipts` and `Campbooks/Newsletters`,
Primary having no mark. Marks SHALL be recognised by exact name,
case-insensitive, in the provider's label listing, and SHALL be created at
the provider by the push that first needs one. Marks SHALL never be carried
as labels in a batch. Every upsert of a received message SHALL carry
`provider_stream`: the stream its single mark names; with no mark, at Gmail
`updates` for `CATEGORY_UPDATES`, `newsletters` for `CATEGORY_PROMOTIONS`,
none for `CATEGORY_SOCIAL` or `CATEGORY_FORUMS`, otherwise `primary`; at
Zoho `primary`; and none when it carries more than one mark. At Gmail, a
category change on a message that keeps its mark SHALL win over the mark:
the upsert carries the category's stream and `stream_mark_stale` true.

#### Scenario: The mark is created on first use
- **WHEN** a member moves a Gmail conversation to Receipts and the mailbox has no `Campbooks/Receipts` label
- **THEN** the push creates the label, the message gets it and `CATEGORY_UPDATES`, and the change is `applied`

#### Scenario: A mark is not a label
- **WHEN** a Gmail message carries `Campbooks/Receipts`, `CATEGORY_UPDATES` and the user label "Clients"
- **THEN** its upsert carries label "Clients" only, and `provider_stream` `receipts`

#### Scenario: A tab change on a marked message
- **WHEN** a member drags a message carrying `Campbooks/Receipts` to Gmail's Promotions tab
- **THEN** the next batch carries it with `provider_stream` `newsletters` and `stream_mark_stale` true

Zoho's message listing names no labels, so the Zoho adapter SHALL read
them through the listing's `labelid` filter: each run, the newest page for
every stream mark; each sweep, every label's messages in the window, a
message missing from a label's listing having lost it. Until a label has
been read this way, the adapter SHALL not take its own index as the
message's labels.

#### Scenario: Zoho stream from a mark
- **WHEN** a member adds `Campbooks/Newsletters` to a recent message in Zoho
- **THEN** the next run's batch carries it with `provider_stream` `newsletters`, without downloading it again

#### Scenario: A mark removed in Zoho
- **WHEN** a member takes `Campbooks/Updates` off a message in Zoho
- **THEN** the next sweep's batch carries it with `provider_stream` `primary`

#### Scenario: A Zoho grant without the labels scope
- **WHEN** a Zoho connection was granted before the labels scope was asked for
- **THEN** its upserts carry no labels and no `provider_stream`, label and stream changes are recorded `skipped` (`scope_missing`), and every other change still syncs

## MODIFIED Requirements

### Requirement: Gmail messages adapter
The Gmail adapter SHALL first read the configured window (default 90 days)
newest first, recording the account's history id before it starts, then
read changes incrementally from that history id. A history id Gmail no
longer has SHALL trigger a resync of the window that emits upserts for new
or changed messages and removals for indexed messages in the window that
are gone. Messages SHALL be fetched as raw MIME; the raw message, text part,
HTML part and attachments SHALL be stored as blobs before the batch that
references them. A text part SHALL be stored only when the message has a
`text/plain` part: none SHALL be made out of its HTML, which conversations
reads as text itself. A change of labels only SHALL be emitted without
downloading the message again. Drafts and chats SHALL be skipped. Stream
marks SHALL be read as the message's provider stream, never as labels.

#### Scenario: Backfill then incremental
- **WHEN** a new Gmail stream has 120 messages in its window and two more arrive during the backfill
- **THEN** the backfill emits the 120 messages over several batches, and the first incremental run emits the two new ones

#### Scenario: Labels mapped
- **WHEN** a message carries `INBOX`, `UNREAD`, `STARRED`, `CATEGORY_UPDATES` and a user label "Clients"
- **THEN** its upsert says in inbox, not read, starred, category `updates`, label `{key: <label id>, name: "Clients"}`, and `provider_stream` `updates`

#### Scenario: Archived in Gmail
- **WHEN** a known message loses its `INBOX` label
- **THEN** the next batch carries its upsert with `in_inbox` false, and the message was not downloaded again

#### Scenario: A tab change in Gmail
- **WHEN** a known message moves from Gmail's Updates tab to Primary
- **THEN** the next batch carries its upsert with `provider_stream` `primary`, and the message was not downloaded again

#### Scenario: A run stops at its byte budget
- **WHEN** a page holds more new mail than one run's byte budget
- **THEN** the run stores what it fetched, keeps its cursor, and the next run continues with the rest; every message is emitted once and downloaded once

#### Scenario: Oversized message
- **WHEN** a message is larger than the configured size limit
- **THEN** its download is abandoned, it is recorded as oversized, no batch carries it, and it is never downloaded again

#### Scenario: Expired history
- **WHEN** `history.list` answers 404 for the stored history id
- **THEN** the stream resyncs its window and emits a removal for an indexed message in the window that Gmail no longer lists

#### Scenario: HTML alone
- **WHEN** a message is a single `text/html` part opening with `<head><meta>` and no `<body>` tag
- **THEN** its upsert carries its HTML part and no text part

### Requirement: Outbound changes to Gmail
The module SHALL consume `conversations.changes_requested` v1 and make each
item's change at Gmail for the stream that feeds the event's inbox: read and
unread through `UNREAD`, star and unstar through `STARRED`, archive and
unarchive through `INBOX`, a provider label by its Gmail label id, and a move
to a stream through its place: Primary, Updates and Newsletters through the
matching Gmail category (`CATEGORY_PERSONAL`, `CATEGORY_UPDATES`,
`CATEGORY_PROMOTIONS`), Receipts through `CATEGORY_UPDATES` and the
`Campbooks/Receipts` mark, the other categories and every other stream mark
removed. A change SHALL be recorded durably, with Gmail's
history id at that moment, before it is attempted, and SHALL be made by the
stream's next run before it reads, with `users.messages.modify`, or
`users.messages.batchModify` for more than ten messages with the same change.
Changes recorded before a run SHALL be merged per message and label, the
latest winning. Every recorded change SHALL end `applied`, `superseded`,
`skipped` or `failed` with a stable reason code and no message content. A
change whose labels the message already has SHALL be recorded `skipped`
(`already_set`) without a call. A label without a provider key, or that
Gmail no longer has, SHALL be recorded as `skipped`. Only `UNREAD`,
`STARRED`, `INBOX`, the categories, stream marks and user labels SHALL ever
be written; a label change naming a system label or a stream mark SHALL be
skipped. A change that would bring a message
Gmail holds in the trash or spam back to the inbox or a category SHALL be
recorded `skipped` (`in_trash`). A change still pending after the batch
retention period SHALL be recorded `failed` (`expired`) before any run takes
it, and never made.

#### Scenario: Archive reaches Gmail
- **WHEN** a member archives a conversation whose message is in Gmail's inbox
- **THEN** the stream runs, Gmail's message loses `INBOX`, and the change is recorded `applied`

#### Scenario: Every intent kind
- **WHEN** members mark read and unread, star and unstar, archive and unarchive, add and remove a Gmail label, and move conversations to Primary, Updates, Newsletters and Receipts
- **THEN** Gmail's labels change as mapped, the move to Receipts adds `Campbooks/Receipts`, and the label without a provider key is recorded `skipped`

#### Scenario: Out of Receipts
- **WHEN** a member moves a conversation from Receipts to Primary
- **THEN** its message loses `Campbooks/Receipts` and `CATEGORY_UPDATES` and gets `CATEGORY_PERSONAL`

#### Scenario: Already in place
- **WHEN** a move to Updates is requested for a message Gmail already holds in the Updates tab
- **THEN** no call is made and the change is recorded `skipped` (`already_set`)

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

### Requirement: Zoho messages adapter
The Zoho adapter SHALL read each tracked folder (Inbox, Sent, Spam, Trash
and the member's folders; never Drafts, Templates, Outbox or Snoozed)
newest first: first the configured window (default 90 days), then the
newest page of every folder each run, and the window again every
`CAMPBOOKS_CONNECTORS__ZOHO__SWEEP_MINUTES` (default 360). New messages SHALL
be fetched as raw MIME and stored like Gmail's; a known message whose
folder, read state, flag, archive state or labels changed SHALL be emitted
again without downloading it. A known message that leaves its folder's listing
and appears in no other SHALL be looked up: archived, moved, or removed.
A message its folder no longer has SHALL be looked for in the account's
other folders before it counts as removed. Zoho failing on one message (a
download or a look-up) SHALL skip that message, retried later with a
backoff from a minute doubling to a day, never stop the run. Metadata SHALL
be HTML-decoded before use, and an answer that does not parse SHALL be
reported by its error category and position only. The upsert SHALL say in inbox
for the Inbox folder when not archived, spam and trash by folder, read
from Zoho's status, starred for any flag, outbound for the Sent folder,
its Zoho labels by id and name except stream marks, its provider stream,
and no categories or thread key; its headers SHALL come from the raw
message.

#### Scenario: Backfill then a new message
- **WHEN** a Zoho stream has messages in its Inbox and Sent folders and one more arrives after the backfill
- **THEN** the backfill emits them all, and the next incremental run emits the new one only

#### Scenario: Bulk signals reach the decide pass
- **WHEN** a newsletter with `List-Unsubscribe`, `List-Id` and `Precedence: bulk` arrives
- **THEN** its upsert carries those headers, read from its raw message

#### Scenario: Archived in Zoho
- **WHEN** a known message is archived in Zoho
- **THEN** the next run emits it with `in_inbox` false, and it was not downloaded again

#### Scenario: Labelled in Zoho
- **WHEN** a member adds the Zoho label "Clients" to a known message
- **THEN** the next run emits it with label `{key: <label id>, name: "Clients"}`, and it was not downloaded again

#### Scenario: Deleted in Zoho
- **WHEN** a known message is deleted for good in Zoho
- **THEN** the next run emits its removal

### Requirement: Outbound changes to Zoho
The module SHALL make members' changes to messages of a Zoho stream, with
the merge, supersede, trash, retry and give-up rules of the Gmail
write-back and without a history stamp (arrival order): read and unread
(`markAsRead`, `markAsUnread`), star and unstar (`setFlag` `important`,
`flag_not_set`), archive (`archiveMails`) and unarchive (`unArchiveMails`
only for a message actually archived, a move to the Inbox folder only for
one in the member's own folders), a Zoho label by its id (`applyLabel`,
`removeLabel`), and a move to a stream by its stream mark (every other mark
removed; Primary removes them all). A message in the Sent folder, spam, the
trash, or a folder the adapter does not read (drafts, outbox, snoozed)
SHALL never be moved to the Inbox: that part of the change is recorded
`skipped` (`not_movable`, or `in_trash`) and the message's state emitted
again. A label change without a provider key Zoho lists SHALL be recorded
`skipped` (`label_not_at_provider`), and a change already in place
`skipped` (`already_set`). After a write, the
index SHALL hold the state written, so the next run emits nothing for it.

#### Scenario: Archive reaches Zoho
- **WHEN** a member archives a conversation whose message is in the Zoho Inbox
- **THEN** Zoho receives `archiveMails` for it, the change is `applied`, and the next run emits no batch for that message

#### Scenario: Unarchive a moved message
- **WHEN** a member unarchives a message that sits in one of their own Zoho folders
- **THEN** Zoho receives `moveMessage` to the Inbox folder and the change is `applied`

#### Scenario: Unarchive a conversation with the member's own reply
- **WHEN** a member unarchives a conversation holding an archived message and their own reply in Sent
- **THEN** Zoho receives `unArchiveMails` for the archived message only, the reply stays in Sent, and its change is `skipped` (`not_movable`)

#### Scenario: A stream move reaches Zoho
- **WHEN** a member moves a Zoho conversation from Updates to Receipts
- **THEN** its message loses `Campbooks/Updates` and gets `Campbooks/Receipts` (created first if Zoho has none), and the next run emits no batch for it

#### Scenario: A Zoho label
- **WHEN** a member takes off a Zoho label the provider knows
- **THEN** Zoho receives `removeLabel` for it and the change is `applied`

### Requirement: Write-backs never echo
Changes that came from Gmail SHALL never be written back to Gmail. After a
write, the adapter's index SHALL hold the labels it set, so that Gmail's
history record of that write produces no change in the next batch. While a
member's change is pending, a run that emits that message SHALL emit it with
the change applied. The one exception: a write that changed a message's
provider stream SHALL be emitted once from the index (no download, no
provider call), so conversations stores where the message now is; that
emission SHALL carry no change conversations sends back.

#### Scenario: The next sync after a write-back
- **WHEN** a member's star has been written to Gmail and the stream runs again
- **THEN** no `connectors.messages_changed` batch carries that message, and no new outbound change is recorded

#### Scenario: A Gmail change still arrives
- **WHEN** a member's archive is written while the same message was starred in Gmail and not yet read
- **THEN** the next batch carries the message starred and archived

#### Scenario: A stream write is reported once
- **WHEN** a move to Primary is written to a message in Gmail's Promotions tab
- **THEN** one batch carries the message with `provider_stream` `primary`, conversations publishes nothing for it, and a later run emits nothing more
