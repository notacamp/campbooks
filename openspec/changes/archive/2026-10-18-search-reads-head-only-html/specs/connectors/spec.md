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
downloading the message again. Drafts and chats SHALL be skipped.

#### Scenario: Backfill then incremental
- **WHEN** a new Gmail stream has 120 messages in its window and two more arrive during the backfill
- **THEN** the backfill emits the 120 messages over several batches, and the first incremental run emits the two new ones

#### Scenario: Labels mapped
- **WHEN** a message carries `INBOX`, `UNREAD`, `STARRED`, `CATEGORY_UPDATES` and a user label "Clients"
- **THEN** its upsert says in inbox, not read, starred, category `updates`, and label `{key: <label id>, name: "Clients"}`

#### Scenario: Archived in Gmail
- **WHEN** a known message loses its `INBOX` label
- **THEN** the next batch carries its upsert with `in_inbox` false, and the message was not downloaded again

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
