# connectors Specification

## Purpose
Links between a workspace and outside accounts: connections and the
capabilities they grant, their encrypted credentials and health, the sync
engine that reads each connection as change streams with cursors, and the
provider adapters (Gmail first) that produce the documents domain modules
apply (ADR 0010).

## Requirements

### Requirement: Credentials stay secret
Refresh and access tokens SHALL be stored only encrypted (AES-256-GCM, key
derived from the instance secret with a purpose label, bound to table,
column and row) and SHALL never be returned by any operation, written to
logs, or carried in events or job payloads. Configuring a provider without
the instance secret SHALL stop startup with an error naming the variable.

#### Scenario: Reading a connection
- **WHEN** a member reads a connection
- **THEN** the response holds its provider, address, capabilities, health and streams, and no token

#### Scenario: Ciphertext moved to another row
- **WHEN** a sealed refresh token is copied into another connection's row
- **THEN** it does not decrypt, and that connection's health becomes `needs_reauth`

### Requirement: Health and re-authorisation
Every connection SHALL have a health of `ok`, `needs_reauth`, `throttled` or
`failing` with a reason. A revoked or expired grant SHALL set `needs_reauth`,
pause its streams and publish `connectors.connection_needs_reauth`. A
provider rate limit SHALL set `throttled` and pause only that stream until
the provider's `Retry-After`. Repeated failures SHALL back the stream off
towards its maximum interval and, past a threshold, set `failing`. Any successful run SHALL
restore `ok`.

#### Scenario: Grant revoked at Google
- **WHEN** Google answers `invalid_grant` while refreshing a connection's token
- **THEN** its health is `needs_reauth`, its streams stop being scheduled, and `connectors.connection_needs_reauth` is published once

#### Scenario: Throttled
- **WHEN** Gmail answers 429 with `Retry-After: 120` during a sync
- **THEN** the stream's health is `throttled`, it is next due 120 seconds later, and other streams are unaffected

### Requirement: Disconnecting
The grantor, or an owner or admin, SHALL be able to disconnect a
connection: the grant SHALL be revoked at the provider, credentials, streams
and indexes deleted, and `connectors.connection_removed` published with the
inbox ids. When a member leaves a workspace, the connections they granted
there SHALL be disconnected the same way. When a workspace is being
deleted, every grant SHALL be revoked and every connection deleted the same
way at once.

#### Scenario: Grantor disconnects
- **WHEN** the grantor disconnects a Google connection
- **THEN** Google's revoke endpoint is called, the connection is gone, and `connectors.connection_removed` lists its inbox id

#### Scenario: Grantor leaves the workspace
- **WHEN** `workspaces.member_left` arrives for a member who granted a connection
- **THEN** that connection is disconnected and revoked

#### Scenario: Workspace deleted
- **WHEN** `workspaces.workspace_deleting` arrives for a workspace with a connection
- **THEN** its grant is revoked and the connection is gone, without waiting for the grace period

### Requirement: Adaptive, fair sync
Each connection and capability SHALL be read as a stream with a cursor. A
stream SHALL be synced by at most one job at a time; requests to sync a
stream that already has a pending run SHALL collapse into it. A run SHALL
read one bounded page and SHALL be scheduled again at once when more remains,
at the minimum interval after a page with changes, and at a doubling
interval up to the maximum while nothing changes. `connectors.sync_now`
SHALL make the stream due now at the minimum interval. Requests to a
provider SHALL draw from a per-provider budget shared by every process.

#### Scenario: Idle mailbox backs off
- **WHEN** a stream's incremental runs find no change several times in a row
- **THEN** its interval doubles each time and stops at the configured maximum

#### Scenario: Sync now collapses
- **WHEN** a member calls sync now twice while the stream's run is pending
- **THEN** one run is pending and the stream's interval is the minimum

#### Scenario: Large backfill does not starve others
- **WHEN** one workspace backfills a large mailbox and another workspace's stream becomes due
- **THEN** the second stream's run is claimed before the first workspace's next page

### Requirement: Gmail messages adapter
The Gmail adapter SHALL first read the configured window (default 90 days)
newest first, recording the account's history id before it starts, then
read changes incrementally from that history id. A history id Gmail no
longer has SHALL trigger a resync of the window that emits upserts for new
or changed messages and removals for indexed messages in the window that
are gone. Messages SHALL be fetched as raw MIME; the raw message, text part,
HTML part and attachments SHALL be stored as blobs before the batch that
references them. A change of labels only SHALL be emitted without
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

### Requirement: The message change stream
Each sync run that finds changes for a messages stream SHALL store one
MessageChangeBatch v1 document (`contracts/message-change-batch.v1.schema.json`)
as a blob of the workspace, keep it attached to a batch record for the
retention period, and publish `connectors.messages_changed`
(`contracts/connectors.messages_changed.v1.schema.json`) in the transaction
that advances the stream's cursor. Event payloads SHALL hold ids and blob
references only. When a connection gains the `messages` capability, the
module SHALL mint the inbox id and publish `connectors.capability_granted`
before any batch for that inbox.

#### Scenario: Batch matches the contract
- **WHEN** a sync run emits a batch
- **THEN** the batch blob validates against the MessageChangeBatch v1 schema and the event payload validates against the messages_changed v1 schema

#### Scenario: Cursor and event together
- **WHEN** a sync run fails after storing blobs but before its transaction commits
- **THEN** the cursor has not moved and no `messages_changed` event exists, so the next run reads the same changes again

#### Scenario: Inbox announced first
- **WHEN** a member completes a Gmail connection
- **THEN** `connectors.capability_granted` with the new inbox id and an account blob is published in the completing transaction, before the stream's first batch

### Requirement: Connections operations
The module SHALL expose start, list, get, disconnect and sync-now
operations under `/api/v1/w/{workspace_id}/connections`, each with an
`operationId` prefixed `connectors.`, an `x-campbooks` block and the scope
`connectors:read` or `connectors:write`. A member SHALL see the
connections they granted; owners and admins SHALL see all of the
workspace's; any other read SHALL be 404. A client token SHALL NOT start a
connection.

#### Scenario: Member lists connections
- **WHEN** a member lists connections in a workspace with their own and a colleague's connection
- **THEN** only their own is listed, while an admin sees both

#### Scenario: Client token starts a connection
- **WHEN** a client token calls start connection
- **THEN** the response is 403 `connection_needs_user`

### Requirement: Connecting an account through the front door
A member SHALL start a connection by naming a provider and capabilities (and
optionally a connection of theirs to extend), and SHALL receive the URL of
the identity module's front door for that consent. The provider round trip,
its PKCE (S256), offline access and expiry are the identity module's, and
the grant SHALL arrive through the grant-intake port, which creates or
refreshes the connection through the one seam
(`create_or_refresh_connection`). Extending a connection SHALL refuse any
other account (`account_mismatch`). With no in-process identity issuer, or a
provider that is not configured, the answer SHALL be 409
`provider_not_configured`. The grant-intake port SHALL tell the identity
module, for a connection to reconnect, its provider and address, and SHALL
answer 404 unless the caller granted it.

#### Scenario: Member connects Gmail
- **WHEN** a member starts a Google connection for `messages`
- **THEN** the answer's `authorize_url` is a `/auth/start/google?link=` URL of this instance, and the grant that comes back through the intake becomes a connection with the member as grantor, health `ok` and a messages stream due now

#### Scenario: Reconnecting someone else's connection
- **WHEN** the identity module asks for the reconnect target of a connection another member granted
- **THEN** the answer is 404

#### Scenario: Same account again
- **WHEN** the member connects the same Google account again in the same workspace, asking for a capability it lacks
- **THEN** the existing connection gains the capability and its credentials are replaced; no second connection exists

#### Scenario: Reconnecting with another account
- **WHEN** a grant extending a connection comes back for a different Google account
- **THEN** it is refused with `account_mismatch` and the connection is unchanged

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
