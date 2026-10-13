# connectors Specification

## Purpose
Links between a workspace and outside accounts: connections and the
capabilities they grant, their encrypted credentials and health, the sync
engine that reads each connection as change streams with cursors, and the
provider adapters (Gmail first) that produce the documents domain modules
apply (ADR 0010).

## Requirements

### Requirement: Connecting an account
A member SHALL start a connection by naming a provider and capabilities and
SHALL receive the provider's authorize URL, which uses the authorization
code flow with PKCE (S256), offline access and incremental consent. The
started authorization SHALL expire after ten minutes. The connection SHALL
be completed only by an authenticated call, from the member who started it
and in the same workspace, carrying the `state` and `code` from the
redirect; any other caller SHALL get 404 and the code SHALL NOT be
exchanged. A provider that is not configured SHALL answer 409
`provider_not_configured`.

#### Scenario: Member connects Gmail
- **WHEN** a member starts a Google connection for `messages` and completes it with the returned state and the code Google issued
- **THEN** a connection exists with the member as grantor, health `ok`, capability `messages`, and a messages stream that is due now

#### Scenario: Someone else completes it
- **WHEN** a different user completes an authorization another member started
- **THEN** the response is 404, no token is requested from Google, and no connection is created

#### Scenario: Expired authorization
- **WHEN** the member completes an authorization more than ten minutes after starting it
- **THEN** the response is 404 and no connection is created

#### Scenario: Same account again
- **WHEN** the member connects the same Google account again in the same workspace, asking for a capability it lacks
- **THEN** the existing connection gains the capability and its credentials are replaced; no second connection exists

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
The module SHALL expose start, complete, list, get, disconnect and sync-now
operations under `/api/v1/w/{workspace_id}/connections`, each with an
`operationId` prefixed `connectors.`, an `x-campbooks` block and the scope
`connectors:read` or `connectors:write`. A member SHALL see the
connections they granted; owners and admins SHALL see all of the
workspace's; any other read SHALL be 404. A client token SHALL NOT start or
complete a connection.

#### Scenario: Member lists connections
- **WHEN** a member lists connections in a workspace with their own and a colleague's connection
- **THEN** only their own is listed, while an admin sees both

#### Scenario: Client token starts a connection
- **WHEN** a client token calls start connection
- **THEN** the response is 403 `connection_needs_user`

### Requirement: Sending through Gmail
On `conversations.send_requested` v1 the consumer `connectors.queue_send`
SHALL record the send, keyed by its send id (a repeated delivery adds
nothing), hold the outgoing message document, and enqueue
`connectors.send_message`. That job SHALL build an RFC 5322 message from
the document (From the connection's account, To, Cc, Bcc, Subject, Date,
`Message-ID`, `In-Reply-To`, `References`, a plain-text part, an HTML
alternative when given, and the attachments) and send it with
`users.messages.send` in the document's thread. When the inbox has no
connected stream, or its grant needs the member again, the send SHALL fail
with `not_connected` or `needs_reauth` without calling Gmail.

#### Scenario: A reply goes into its Gmail thread
- **WHEN** a reply's document names a thread key
- **THEN** Gmail receives one `messages.send` with that `threadId`, and the raw message carries the `In-Reply-To`, `References` and `Message-ID` of the document

### Requirement: Sends are idempotent
Every call to Gmail SHALL be preceded by an attempt committed on its own.
When an attempt ends unclearly (a timeout, a 5xx, the worker died), the next
attempt SHALL wait at least 30 seconds and then look the message up by its
`Message-ID`, recording it as sent if Gmail has it, and sending only if it
does not. A definite refusal SHALL fail the send with `rejected` at once. A
send whose last attempt ends unclearly SHALL fail with `unknown_outcome`.

#### Scenario: A timeout after Gmail accepted the message
- **WHEN** Gmail accepts a message but the answer never arrives, and the job runs again
- **THEN** the job finds the message by its `Message-ID` and Gmail receives no second `messages.send`

#### Scenario: The same intent delivered twice
- **WHEN** `conversations.send_requested` is delivered twice for one send id
- **THEN** one send is recorded and Gmail receives one `messages.send`

#### Scenario: A refused message
- **WHEN** Gmail answers 400 to `messages.send`
- **THEN** the send fails with `rejected` and is never retried

### Requirement: The sent message comes back once
After Gmail accepts a message, the job SHALL, in one transaction, index it
like a synced message, store and announce a `MessageChangeBatch` holding it
(`connectors.messages_changed`), mark the send sent, release its document,
publish `connectors.send_completed` v1 with the provider's id, and make the
stream due at once. The next sync SHALL recognise the message and not emit
it as new.

#### Scenario: Sync after a send
- **WHEN** the stream syncs after a send and Gmail's history lists the sent message as added
- **THEN** no second upsert of that message is emitted as new and the conversation still has one copy
