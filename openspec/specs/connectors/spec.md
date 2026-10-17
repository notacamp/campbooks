# connectors Specification

## Purpose
Links between a workspace and outside accounts: connections and the
capabilities they grant, their encrypted credentials and health, the sync
engine that reads each connection as change streams with cursors, and the
provider adapters (Gmail, Zoho Mail) that produce the documents domain modules
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
references only. When a connection gains a `messages` stream (the
capability, or another mailbox of its grant), the module SHALL mint the
inbox id and publish `connectors.capability_granted` before any batch for
that inbox.

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
answer 404 unless the caller granted it. It SHALL also tell the identity
module the connection of a provider account in a workspace, only when the
person it names granted it there (none otherwise, or once they are no
longer a member).

#### Scenario: Member connects Gmail
- **WHEN** a member starts a Google connection for `messages`
- **THEN** the answer's `authorize_url` is a `/auth/start/google?link=` URL of this instance, and the grant that comes back through the intake becomes a connection with the member as grantor, health `ok` and a messages stream due now

#### Scenario: Whose connection it is
- **WHEN** the identity module asks for the connection of a Google account a member granted in their workspace
- **THEN** it learns its id when it names that member, and nothing for another member, another account, another workspace or someone who is not a member

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
Every call to Gmail SHALL be preceded by an attempt committed on its own,
which marks the send unclear (it may be out) until Gmail gives a definite
answer: an acceptance, or a refusal of the send itself (any 4xx; a 2xx whose
answer cannot be read counts as unclear, never as a refusal). While a send
is unclear, the next attempt SHALL look the message up by its `Message-ID`
first, recording it as sent if Gmail has it and clearing the mark if it does
not; no attempt SHALL call Gmail sooner than the HTTP timeout plus 30
seconds after the previous one. A definite refusal SHALL fail the send with
`rejected` at once. Any failure while the send is unclear SHALL be
`unknown_outcome`. A send whose attempts run out, or that cannot go on,
SHALL be settled: `unknown_outcome` if unclear, else `unavailable`; and a
check enqueued behind each send (it runs only once no earlier job for the
send is alive, 30 minutes on) SHALL settle a send whose job died the same
way, after a lookup when it is unclear.

#### Scenario: A timeout after Gmail accepted the message
- **WHEN** Gmail accepts a message but the answer never arrives, and the job runs again
- **THEN** the job finds the message by its `Message-ID` and Gmail receives no second `messages.send`

#### Scenario: The same intent delivered twice
- **WHEN** `conversations.send_requested` is delivered twice for one send id
- **THEN** one send is recorded and Gmail receives one `messages.send`

#### Scenario: A refused message
- **WHEN** Gmail answers 400 to `messages.send`
- **THEN** the send fails with `rejected` and is never retried

#### Scenario: An answer that cannot be read
- **WHEN** Gmail accepts a message but its answer does not parse
- **THEN** the next attempt finds it by its `Message-ID` and Gmail receives one `messages.send`

#### Scenario: The worker died after the call
- **WHEN** an attempt was recorded with no outcome, and the grant then needs the member
- **THEN** the send fails with `unknown_outcome`, never as not sent

#### Scenario: Out of attempts before any call
- **WHEN** a send's last attempt cannot call Gmail (no budget, no token)
- **THEN** it fails with `unavailable` and its draft is editable again

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

### Requirement: Zoho Mail connections in their data centre
The module SHALL connect Zoho Mail accounts when `CAMPBOOKS_ZOHO__CLIENT_ID`
and a client secret are set, asking in the grant for
`ZohoMail.accounts.READ`, `ZohoMail.folders.READ` and `ZohoMail.messages.ALL`.
A Zoho grant SHALL be taken only when its issuer is one of Zoho's accounts
servers (`.com`, `.eu`, `.in`, `.com.au`, `.jp`, `.ca`, `.sa`, `.uk`), or, in
development only, the server `CAMPBOOKS_ZOHO__BASE_URL` names; any other
SHALL be refused with `unknown_data_centre`. Its tokens SHALL be refreshed
and revoked at that accounts server (the token in the body first, the
query only if Zoho refuses that; a refusal is never taken as revoked), and its mail read and written at that
data centre's mail host (`accounts.zoho.<x>` → `mail.zoho.<x>`), never at a
host taken from a request or a provider's answer. A refresh Zoho refuses
(`invalid_code` or `invalid_grant`, even in a 200 answer) SHALL set
`needs_reauth`.

#### Scenario: A European account
- **WHEN** a grant issued by `https://accounts.zoho.eu` is connected and its stream runs
- **THEN** its token is refreshed at `https://accounts.zoho.eu/oauth/v2/token` and its mail is read from `https://mail.zoho.eu/api/`

#### Scenario: A made-up data centre
- **WHEN** a Zoho grant names the issuer `https://accounts.evil.example`
- **THEN** no connection is created and the answer is `unknown_data_centre`

### Requirement: Several mailboxes per connection
A connection SHALL have one `messages` stream per mailbox its grant reaches,
each with its own inbox id and its mailbox's address. For Zoho, the
mailboxes SHALL be the login's mail accounts (`GET /api/accounts`), read
when the grant is taken; a refreshed grant SHALL add streams for mail
accounts that are new and keep the others. `connectors.capability_granted`
SHALL be published once per new mailbox stream, with a connection-account
document naming that mailbox's address.

#### Scenario: One Zoho login, two mail accounts
- **WHEN** a member connects a Zoho login whose accounts list `sam@example.net` and `billing@example.net`
- **THEN** the connection has two messages streams, two `capability_granted` events name two inbox ids with those addresses, and each stream's batches name only its own inbox

#### Scenario: Two Zoho logins in one workspace
- **WHEN** a member connects two different Zoho logins in the same workspace
- **THEN** there are two connections, each with its own credentials and health

### Requirement: Zoho messages adapter
The Zoho adapter SHALL read each tracked folder (Inbox, Sent, Spam, Trash
and the member's folders; never Drafts, Templates, Outbox or Snoozed)
newest first: first the configured window (default 90 days), then the
newest page of every folder each run, and the window again every
`CAMPBOOKS_CONNECTORS__ZOHO__SWEEP_MINUTES` (default 360). New messages SHALL
be fetched as raw MIME and stored like Gmail's; a known message whose
folder, read state, flag or archive state changed SHALL be emitted again
without downloading it. A known message that leaves its folder's listing
and appears in no other SHALL be looked up: archived, moved, or removed.
A message its folder no longer has SHALL be looked for in the account's
other folders before it counts as removed. Zoho failing on one message (a
download or a look-up) SHALL skip that message, retried later with a
backoff from a minute doubling to a day, never stop the run. Metadata SHALL
be HTML-decoded before use, and an answer that does not parse SHALL be
reported by its error category and position only. The upsert SHALL say in inbox
for the Inbox folder when not archived, spam and trash by folder, read
from Zoho's status, starred for any flag, outbound for the Sent folder,
with no categories, labels or thread key; its headers SHALL come from the
raw message.

#### Scenario: Backfill then a new message
- **WHEN** a Zoho stream has messages in its Inbox and Sent folders and one more arrives after the backfill
- **THEN** the backfill emits them all, and the next incremental run emits the new one only

#### Scenario: Bulk signals reach the decide pass
- **WHEN** a newsletter with `List-Unsubscribe`, `List-Id` and `Precedence: bulk` arrives
- **THEN** its upsert carries those headers, read from its raw message

#### Scenario: Archived in Zoho
- **WHEN** a known message is archived in Zoho
- **THEN** the next run emits it with `in_inbox` false, and it was not downloaded again

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
one in the member's own folders). A message in the Sent folder, spam, the
trash, or a folder the adapter does not read (drafts, outbox, snoozed)
SHALL never be moved to the Inbox: that part of the change is recorded
`skipped` (`not_movable`, or `in_trash`) and the message's state emitted
again. Label changes and moves between streams SHALL be recorded `skipped`
(`label_not_at_provider`, `no_provider_equivalent`). After a write, the
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

### Requirement: Sending through Zoho
A send from an inbox fed by a Zoho stream SHALL go through Zoho's send API
from the stream's mailbox, with To, Cc, Bcc, subject, the HTML alternative
(or the text) and the attachments, uploaded first. A reply whose
`In-Reply-To` names a message of the stream SHALL be sent as Zoho's reply to
that message. The idempotency rules of Gmail sending SHALL hold, with one
difference: while a send is unclear, the next attempt SHALL look in the
mailbox's Sent folder and Outbox for a message sent between 30 seconds
before that attempt and the HTTP timeout plus 30 seconds after it, with
the same subject, To, Cc, attachment flag and start of text, not already
another send's. Exactly one such message SHALL record the send as sent;
none SHALL prove it was not sent; several, a copy without a time, or a
mailbox whose Sent folder cannot be found SHALL leave it unclear. When
Zoho's answer to a send names no message, the job SHALL claim the Sent
copy it finds this way, so no later look-up takes it for another send. Once Zoho accepts, the sent message SHALL be fetched, indexed and
announced in the job's transaction like Gmail's; when it cannot be fetched
the send SHALL still be `sent` and the stream due at once.

#### Scenario: A reply goes out as a Zoho reply
- **WHEN** a member replies to a message of a Zoho inbox
- **THEN** Zoho receives one reply call naming that message, and the sent copy joins the same conversation

#### Scenario: A timeout after Zoho accepted the message
- **WHEN** Zoho accepts a message but the answer is lost, and the job runs again
- **THEN** the job finds it in the Sent folder and Zoho receives no second send

#### Scenario: A follow-up with the same subject
- **WHEN** an attempt never reached Zoho and, two minutes later, the member sends another message with the same subject and recipient from another client
- **THEN** that message is not taken for the send, which is sent

#### Scenario: Two matching copies
- **WHEN** two sent messages match an unclear attempt
- **THEN** the send stays unclear, is never sent again, and ends `unknown_outcome`

### Requirement: Zoho request budget
Requests to Zoho SHALL draw from a budget per Zoho login,
`CAMPBOOKS_CONNECTORS__ZOHO__REQUESTS_PER_MINUTE` (default 60), shared by
every process; a run SHALL take its listings and a share of downloads
before it starts and wait for the budget otherwise. Zoho's 429, or an
answer coded `TOO_MANY_REQUESTS` or as a throttle limit, SHALL throttle only
that stream until `Retry-After`.

#### Scenario: One login's backfill does not spend another's
- **WHEN** one Zoho login's budget is spent and another login's stream is due
- **THEN** the second stream runs at once

### Requirement: Inline parts keep their Content-ID
When the Gmail adapter stores a message's attachments, each one SHALL carry
the part's `Content-ID` without its angle brackets (as `content_id` in the
MessageChangeBatch v1 attachment, optional, at most 255 characters; a
longer one is left out), so that the body's `cid:` images can be resolved.

#### Scenario: An inline logo
- **WHEN** a message's HTML part shows `cid:logo@example.com` and a `multipart/related` part has `Content-ID: <logo@example.com>`
- **THEN** the upsert's attachment for that part has `content_id` `logo@example.com` and `inline` true

### Requirement: Mailboxes screen
Settings SHALL have a Mailboxes page listing the workspace's connections the
person may see (`connectors.list_connections`), each with its provider, its
address, who connected it ("you", from the server's `granted_by_you`, never
by matching an email, or the member's name) and its health in
plain words: `ok` "Syncing", `needs_reauth` "Needs you to sign in again"
(or the grantor's name), `throttled` "Paused by <provider> for a few
minutes", `failing` "Not syncing — we'll keep trying". The page SHALL offer:

- Connect for each of the server's `mailbox_providers`, through
  `identity.start_link` with `connect` and this page as `return_to`, asking
  "Also use this account to sign in" (ticked by default, untick it for a
  shared mailbox), passed as `sign_in`;
- Reconnect, on the person's own connections that need re-authorisation,
  through `identity.start_link` with `reconnect`;
- Disconnect (`connectors.disconnect`), only on the person's own
  connections, behind an inline confirmation saying that syncing stops and
  access is given up at the provider, asking what becomes of its mail in
  Campbooks ("Delete its mail from Campbooks", the default, or "Keep its
  history, read-only"), that the mail stays at the provider either way, and
  what becomes of the way in: that it goes too only when `identity.list_ways_in`
  records this very connection as the only mailbox of a way in a connect
  created and it is not the last way in, and otherwise that how the person
  signs in does not change (never decided from an address), and nothing
  until the ways in and passkeys are known. Its cancel SHALL read "Don't
  disconnect". A disconnect that fails SHALL be said as a failure, never
  shown as done; one that finds nothing (404) SHALL be said by the page, as
  the row leaves with the list's next answer; focus
  SHALL return to Disconnect when the confirmation closes and the row stays,
  and move to the result once it is gone;
- "Sign in again" (a fresh sign-in, back to this page) when starting is
  refused with `step_up_required`.

Back from the front door, the page SHALL say once that the mailbox is
connected (`connected=`) or explain the refusal (`link_error=`) for every
code the front door and the grant intake return, with a generic message for
any other, and SHALL clear the query. Keys: ⇧C focuses Connect, J and K
choose a mailbox, R reconnects it and ⌫ asks to disconnect it, each shown on
its button. The page SHALL be in en, pt-PT, es-ES and fr, show loading,
empty and error states, fit 375px, and pass axe in light and dark.

#### Scenario: Connect Zoho Mail
- **WHEN** the server lists `google` and `zoho` as mailbox providers and the person presses "Connect Zoho Mail"
- **THEN** `identity.start_link` is called with `connect`, `zoho`, the workspace and this page as `return_to`, and the browser goes to the URL it returns

#### Scenario: Back connected
- **WHEN** the page opens with `?connected=<id>` and the list has that connection
- **THEN** a status says the mailbox is connected, the list shows it as Syncing, and the address bar loses the query

#### Scenario: Refusal explained
- **WHEN** the page opens with `?link_error=account_connected_by_another_member`
- **THEN** an alert explains that another member already connected that mailbox here

#### Scenario: Last sign-in too old
- **WHEN** starting a connect is refused with 403 `step_up_required`
- **THEN** "Sign in again first" is shown, and its button signs in again with `max_age=600` and comes back to the page

#### Scenario: Disconnect deletes the mail by default
- **WHEN** the person disconnects their own mailbox and confirms without changing the choice
- **THEN** `connectors.disconnect` is called with `mail=delete` and the row is gone

#### Scenario: The way in goes with a connect-only mailbox
- **WHEN** the person opens the disconnect confirmation of a connection that `identity.list_ways_in` records as the only mailbox of a way in a connect created, and they have another way in
- **THEN** the confirmation says it also stops being a way to sign in

#### Scenario: A disconnect that finds nothing
- **WHEN** `connectors.disconnect` answers 404 and the list no longer has the connection
- **THEN** the row goes, an alert on the page says the mailbox was not found, and nothing says it is disconnected

#### Scenario: Keep the history
- **WHEN** the person chooses "Keep its history, read-only" and confirms
- **THEN** `connectors.disconnect` is called with `mail=keep`

#### Scenario: A colleague's mailbox
- **WHEN** an owner sees a member's connection
- **THEN** the row offers no Disconnect

#### Scenario: A shared mailbox
- **WHEN** a member opens the disconnect confirmation of a mailbox they connected without signing in with it
- **THEN** both choices are offered, and the confirmation says how they sign in does not change

#### Scenario: A colleague's mailbox needs re-authorisation
- **WHEN** an owner sees a member's connection with health `needs_reauth`
- **THEN** the row names the member who must sign in again and offers no Reconnect

### Requirement: Only the member who connected a mailbox disconnects it
Disconnecting SHALL be allowed only to the member who connected the
account. Owners and admins see it (and may sync it now), but SHALL get 403
`disconnect_owner_only`. A client token gets 403 `connection_needs_user`.
Who may see it stays the grantor, owners and admins, and client tokens.

#### Scenario: An admin disconnects someone's mailbox
- **WHEN** a workspace admin asks to disconnect a member's connection
- **THEN** the answer is 403 `disconnect_owner_only` and nothing is removed

### Requirement: The mail of a removed connection is deleted unless kept
`DELETE …/connections/{id}` SHALL take `mail=delete` (the default) or
`mail=keep`. `connectors.connection_removed` SHALL carry the choice and the
member who had connected it
(`contracts/connectors.connection_removed.v1.schema.json`). When a member
leaves or is removed, their connections SHALL be removed with the choice
made by the owner or admin who removed them (`delete` when they left on
their own).

#### Scenario: Keep on disconnect
- **WHEN** the grantor disconnects with `mail=keep`
- **THEN** `connectors.connection_removed` says `"mail": "keep"`

### Requirement: The dispatcher only reads and leases the schedule
The dispatcher SHALL work in the maintenance scope (`begin_maintenance`,
allowed for it under `[[access.maintenance_scope]]` in `architecture.toml`),
in which it SHALL be able to read the scheduling index and lease due
streams, and nothing else: no kernel table admits that scope, so it SHALL
enqueue each stream's run in the stream's workspace, set for that enqueue
only (`jobs::enqueue_to_in`) in the same transaction as the lease, so that
nothing after it runs in the last stream's workspace. It SHALL NOT be able
to insert or delete schedule rows, and a transaction that set neither a
workspace nor the maintenance scope SHALL see no schedule row; the kernel
scope SHALL NOT admit it to the schedule either.

#### Scenario: The policies
- **WHEN** the database's policies are listed
- **THEN** no module policy admits the kernel scope, the only module policies admitting the maintenance scope are the schedule's `FOR SELECT` and `FOR UPDATE` ones and the needs backfill's, and none admits a transaction merely for setting no workspace

#### Scenario: Nothing set
- **WHEN** a transaction of the runtime role sets no workspace and no scope
- **THEN** it sees no row of the scheduling index

#### Scenario: Due streams of two workspaces
- **WHEN** streams of two workspaces are due and the dispatcher ticks, as the runtime role
- **THEN** each stream's row is leased and its run is enqueued in its own workspace

#### Scenario: No workspace left set
- **WHEN** the dispatcher has enqueued the runs of streams of two workspaces in its transaction
- **THEN** the transaction works in no workspace

### Requirement: The receiving provider's authentication results
The mail adapters SHALL carry, with each message, the receiving provider's
own `Authentication-Results` (RFC 8601), unfolded: the topmost such header in
the message's own order, and only when it is valid UTF-8 and its authserv-id
is one the provider is known to stamp (Gmail: `mx.google.com`; Zoho Mail:
none until a real Zoho message has shown which header Zoho adds). Any other
`Authentication-Results`, including one a sender wrote into the message,
SHALL be dropped, and a message whose topmost one is not the provider's, or
is not valid UTF-8, SHALL carry none.

#### Scenario: Gmail's own results
- **WHEN** a Gmail mailbox receives a message whose topmost `Authentication-Results` is `mx.google.com; dkim=pass …; dmarc=pass … header.from=github.com`
- **THEN** the message change carries that value

#### Scenario: Results a sender wrote
- **WHEN** a message's topmost `Authentication-Results` names another server, or a Zoho mailbox receives one naming `mx.google.com` or `mx.zohomail.com`
- **THEN** the message change carries no authentication results

#### Scenario: A topmost header that is not UTF-8
- **WHEN** the topmost `Authentication-Results` holds a byte that is not UTF-8, above Gmail's own
- **THEN** the message change carries no authentication results

### Requirement: A send or a change is made only while it stands
The connector SHALL NOT take `conversations.send_requested` or
`conversations.changes_requested` as proof that a member asked for
anything. Before it records a send, and again before every call to the
provider for it (a send or the look-up of an unclear attempt), it SHALL ask
the conversations module, through the kernel's outbound-requests port,
whether the send stands, and SHALL act only when it does, from the inbox the
event and the recorded send name. The document it sends SHALL be the one
that port returns (and the one it recorded), never one an event names.
Before it records a change, or asks the provider for a history stamp, it
SHALL ask the same port which of the request's changes its conversation
shows, and SHALL record only those. A request or a send that does not stand
SHALL complete as a no-op with a WARN line carrying ids and counts only, and
SHALL NOT be retried; a recorded send that stops standing SHALL be settled
`unavailable` (or `unknown_outcome` after an attempt that may have gone)
without calling the provider. A port that cannot answer SHALL make the
delivery or the job retry, never act on a guess.

#### Scenario: A forged send for a draft never released
- **WHEN** a transaction working in a workspace appends `conversations.send_requested` for a send the conversations module never released
- **THEN** no send is recorded, the provider receives no call, and no `connectors.send_completed` is published

#### Scenario: A forged send naming another inbox
- **WHEN** the event names an inbox other than the one the send was released from
- **THEN** no send is recorded and nothing is sent

#### Scenario: A forged send naming other content
- **WHEN** the event names a document other than the one the conversations module recorded for the send
- **THEN** the provider receives the recorded document only

#### Scenario: A send that stops standing before its call
- **WHEN** a recorded send's draft is no longer being sent (its author left, it failed) when its job runs
- **THEN** the provider receives no call and the send fails with `unavailable`

#### Scenario: A replay after the send
- **WHEN** `conversations.send_requested` is delivered again after the send went out, while its draft is still sending or once it is gone
- **THEN** nothing more is recorded or sent

#### Scenario: A forged change request
- **WHEN** a transaction working in a workspace appends `conversations.changes_requested` for changes no conversation shows
- **THEN** no outbound change is recorded, the provider is asked for no stamp and receives no write

#### Scenario: Only the changes a conversation shows
- **WHEN** a request names one change its conversation shows and one it does not
- **THEN** only the first is recorded and made at the provider
