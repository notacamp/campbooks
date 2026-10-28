# connectors Specification

## Purpose
Links between a workspace and outside accounts: connections and the
capabilities they grant, their encrypted credentials and health, the sync
engine that reads each connection as change streams with cursors, and the
provider adapters (Gmail, Zoho Mail) that produce the documents domain modules
apply (ADR 0010).

## Requirements

### Requirement: Credentials stay secret
Refresh tokens, access tokens and mailbox passwords SHALL be stored only
encrypted (AES-256-GCM, key derived from the instance secret with a purpose
label, bound to table, column and row) and SHALL never be returned by any
operation, written to logs, or carried in events or job payloads. Configuring
a provider without the instance secret SHALL stop startup with an error
naming the variable.

#### Scenario: Reading a connection
- **WHEN** a member reads a connection
- **THEN** the response holds its provider, address, capabilities, health and streams, and no token or password

#### Scenario: Ciphertext moved to another row
- **WHEN** a sealed refresh token is copied into another connection's row
- **THEN** it does not decrypt, and that connection's health becomes `needs_reauth`

#### Scenario: Password moved to another row
- **WHEN** a sealed IMAP password is copied into another connection's row
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
from the shared provider budget: before the push, what it plans (both
profile reads, the history pages it may read, a `batchModify` per chunk and
each single message's calls), then, once it is done, what its calls really
spent, counted call by call: what it did not spend SHALL be given back
(never filling the budget past its two seconds' worth), and what it spent
beyond the plan (a batch Gmail refused, a group the later-writer check
split) SHALL be owed, taking the budget below zero until the rate pays it
back. A run planned to cost more than the budget holds SHALL wait for a full
budget and owe the rest, so no unit goes uncounted whatever rate is
configured. A change Gmail refuses for good, or that
has met trouble eight times, SHALL be recorded `failed` with a reason code
and no content, and the message's Gmail state SHALL be emitted again. A
message Gmail no longer has SHALL be recorded `skipped` (`message_gone`).

#### Scenario: Throttled, then written
- **WHEN** Gmail answers 429 with `Retry-After` to a write
- **THEN** the change stays pending, the stream is throttled until then, and the next run writes it

#### Scenario: A message that refuses changes
- **WHEN** Gmail refuses the change for one message of a conversation
- **THEN** the other messages are changed, that one is recorded `failed` (`refused`), and conversations is told its Gmail state

#### Scenario: A refused batch is owed
- **WHEN** a push plans one `batchModify` of 12 archives and Gmail refuses the batch
- **THEN** it takes 60 units before writing (both profiles, four history pages, the batch) and, once each message is archived on its own, settles the 112 it spent (one profile read before, the batch, twelve modifies, one after): 52 more are owed

#### Scenario: A push spends less than planned
- **WHEN** a push of one batch reads one history page and Gmail takes the batch
- **THEN** the units it did not spend go back to the budget at once

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
`ZohoMail.accounts.READ`, `ZohoMail.folders.READ`, `ZohoMail.messages.ALL`
and `ZohoMail.tags.ALL` (the member's labels and the stream marks; a
connection granted without it keeps syncing without labels until the member
connects again), and, while `CAMPBOOKS_CONNECTORS__ZOHO_CALENDAR` is on,
`ZohoCalendar.calendar.READ` and `ZohoCalendar.event.ALL`.
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
- **THEN** the next sweep emits it with label `{key: <label id>, name: "Clients"}`, and it was not downloaded again

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
minutes", `failing` "Not syncing — we'll keep trying". An IMAP mailbox whose
address is not verified SHALL also say so: "Checking your address…" while
`address_check` is `checking`, and "We couldn't verify this address: sending
stays off" with "Try again" (`connectors.check_address`, the grantor only)
when it is `failed` or was never checked, so the member knows sending is not
available from it. The page SHALL offer:

- "Add a mailbox" (the Connecting a mailbox by its address requirement),
  whose provider sign-ins go through `identity.start_link` with `connect` and
  this page as `return_to`, asking "Also use this account to sign in"
  (ticked by default, untick it for a shared mailbox), passed as `sign_in`;
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

Right after an IMAP mailbox is connected, the dialog SHALL say it is
connected, and, when its address is not verified, show "Checking your
address…" until the connection's `address_check` changes, then "Verified" or
"We couldn't verify this address: sending stays off" with "Try again".

Back from the front door, the page SHALL say once that the mailbox is
connected (`connected=`) or explain the refusal (`link_error=`) for every
code the front door and the grant intake return, with a generic message for
any other, and SHALL clear the query. Keys: ⇧C opens Add a mailbox, J and K
choose a mailbox, R reconnects it and ⌫ asks to disconnect it, each shown on
its button. The page SHALL be in en, pt-PT, es-ES and fr, show loading,
empty and error states, fit 375px, and pass axe in light and dark.

#### Scenario: Connect Zoho Mail
- **WHEN** the server lists `google` and `zoho` as mailbox providers, the person adds `ana@zohomail.com`, which discovery routes to Zoho, and presses "Connect Zoho Mail"
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

#### Scenario: An address that could not be checked
- **WHEN** the person's IMAP mailbox has `address_check` `failed`
- **THEN** its row says sending stays off until the address is verified and offers "Try again", which calls `connectors.check_address`

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
provider for it, it SHALL ask the conversations module, through the
kernel's outbound-requests port, whether the send stands, from the inbox
the event and the recorded send name, and SHALL send only while it does.
The document it sends SHALL be the one that port returns (and the one it
recorded), never one an event names. A send that no longer stands SHALL NOT
be sent: an unclear earlier attempt SHALL still be looked up (a read), by
the port's document whenever the port returns the send, and one found SHALL
be recorded sent without indexing the bytes this module built; otherwise
the send SHALL be settled `unavailable` without calling the provider. A recorded send SHALL use a stream only of its own inbox, and
fail `not_connected` otherwise. Every call to the provider SHALL be
preceded by an atomic claim of the attempt (the send still pending, no
other attempt recorded since it was read), so two jobs of one send never
both call; the unclear mark SHALL be cleared only while the last attempt is
still the one the clearing run read or made. Before it records a change, or asks the provider for a history
stamp, the connector SHALL ask the same port which of the request's items
the conversations module recorded under that event's id, and SHALL record
only those. A request or a send that does not stand SHALL complete as a
no-op with a WARN line carrying ids and counts only, and SHALL NOT be
retried. A port that cannot answer SHALL make the delivery or the job
retry, never act on a guess.

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

#### Scenario: A send row pointed at another mailbox
- **WHEN** a recorded send names the stream of an inbox other than its own
- **THEN** the provider receives no call and the send fails with `not_connected`

#### Scenario: Two jobs of one send
- **WHEN** two jobs of the same send run side by side
- **THEN** the provider receives one send

#### Scenario: A replay after the send
- **WHEN** `conversations.send_requested` is delivered again after the send went out, while its draft is still sending or once it is gone
- **THEN** nothing more is recorded or sent

#### Scenario: A forged change request
- **WHEN** a transaction working in a workspace appends `conversations.changes_requested` that the conversations module never recorded
- **THEN** no outbound change is recorded, the provider is asked for no stamp and receives no write

#### Scenario: Only the changes recorded with the request
- **WHEN** a request names one change the conversations module recorded under its id and one it did not
- **THEN** only the first is recorded and made at the provider

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

### Requirement: Outbound changes are bound to the request they were recorded for
When the connector records an outbound change (after the outbound-requests
port confirmed its request), it SHALL store, for each row it inserted, a MAC
under the binding key derived from the instance secret, over the
workspace, the row, its stream, its request, the provider message id, the
kind and the labels to add and remove. Before a stream's run makes any
change at the provider it SHALL verify each pending change's MAC; a change
whose MAC does not verify, or that has none, SHALL NOT be made: it SHALL end
`skipped` with the reason `unverified`, its message SHALL be re-read from
the provider so Campbooks shows the provider's state, and a WARN line
carrying ids and a count only SHALL be logged. Without an instance secret
no change SHALL be made at the provider.

#### Scenario: A change planted with SQL
- **WHEN** a transaction of the runtime role in a workspace inserts a pending outbound change no member asked for
- **THEN** the next run writes nothing to the provider and the change ends `skipped` (`unverified`)

#### Scenario: A member's change edited with SQL
- **WHEN** a member's recorded star is turned into an archive before the run
- **THEN** the run writes nothing to the provider for it and it ends `skipped` (`unverified`)

#### Scenario: A member's change as recorded
- **WHEN** a member stars a message and the connector records the change
- **THEN** the run makes it at the provider

### Requirement: Token refresh never waits on a lock
An expired access token SHALL be refreshed once per connection however many
runs need it: the run that takes the connection's refresh lock (only ever
tried, never waited on) SHALL refresh and store it, and the others SHALL use
the stored token once it is there, looking again every 100 ms for at most
the provider call's limit and five seconds more, or refresh themselves if
the lock is freed without a token. Waiting for another run's refresh, within
the provider call's limit, SHALL never fail a run or use up one of its
attempts; a refresh still running past that limit and five seconds more
fails the waiting run as the provider being unavailable.

#### Scenario: A send during a slow refresh
- **WHEN** a sync is refreshing a connection's token and Google takes 1.5 seconds to answer, and a send of the same connection runs meanwhile on connections whose `lock_timeout` is 200 ms
- **THEN** the send uses the refreshed token and is sent on its first attempt, and Google is asked for a token once

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
Zoho's message listing names no labels, so the Zoho adapter SHALL read
them through the listing's `labelid` filter: each run, the newest page for
every stream mark; each sweep, every label's messages in the window, a
message missing from a label's listing having lost it. Until a label has
been read this way, the adapter SHALL not take its own index as the
message's labels.

#### Scenario: The mark is created on first use
- **WHEN** a member moves a Gmail conversation to Receipts and the mailbox has no `Campbooks/Receipts` label
- **THEN** the push creates the label, the message gets it and `CATEGORY_UPDATES`, and the change is `applied`

#### Scenario: A mark is not a label
- **WHEN** a Gmail message carries `Campbooks/Receipts`, `CATEGORY_UPDATES` and the user label "Clients"
- **THEN** its upsert carries label "Clients" only, and `provider_stream` `receipts`

#### Scenario: A tab change on a marked message
- **WHEN** a member drags a message carrying `Campbooks/Receipts` to Gmail's Promotions tab
- **THEN** the next batch carries it with `provider_stream` `newsletters` and `stream_mark_stale` true

#### Scenario: Zoho stream from a mark
- **WHEN** a member adds `Campbooks/Newsletters` to a recent message in Zoho
- **THEN** the next run's batch carries it with `provider_stream` `newsletters`, without downloading it again

#### Scenario: A mark removed in Zoho
- **WHEN** a member takes `Campbooks/Updates` off a message in Zoho
- **THEN** the next sweep's batch carries it with `provider_stream` `primary`

#### Scenario: A Zoho grant without the labels scope
- **WHEN** a Zoho connection was granted before the labels scope was asked for
- **THEN** its upserts carry no labels and no `provider_stream`, label and stream changes are recorded `skipped` (`scope_missing`), and every other change still syncs

### Requirement: Sends and changes go only where they were bound to go
The connector SHALL answer the kernel's outbound-destinations port: an
inbox's destination is its `messages` stream, that stream's connection and
its mailbox at the provider, and no destination when no stream, or more than
one, claims the inbox. Before it records a send, and again before every call
to the provider for it, the connector SHALL pass the outbound-requests port
the destination it would send through (the recorded send's stream, the
stream's connection and its mailbox), and SHALL send only while the port says
the send stands for it. When it records an outbound change it SHALL bind
each row, with a MAC labelled `campbooks/connectors.outbound-change/v2`, to
the stream, the stream's connection and its mailbox as resolved then (the
inbox's one stream; nothing is recorded when two claim it); a run SHALL make
a change only when its MAC verifies for the connection and mailbox the run
loaded. A send refused this way SHALL fail `unavailable` without a provider
call, and a change SHALL end `skipped` (`unverified`) with its message
re-read; each with a WARN line carrying ids and counts only, and neither
retried.

#### Scenario: A stream repointed after a send was released
- **WHEN** SQL points a released send's stream at another mailbox before the connector sends it
- **THEN** the provider receives no call and the send fails `unavailable`, with no job left retrying it

#### Scenario: A stream repointed before the send is recorded
- **WHEN** SQL points the inbox's stream at another mailbox between the release and the connector recording the send
- **THEN** no send is recorded and nothing is sent

#### Scenario: A send released where it goes
- **WHEN** a send is released for the inbox's stream as it is
- **THEN** it is sent once

#### Scenario: A stream repointed after a change was recorded
- **WHEN** a member archives a message and SQL then points the stream at another mailbox before the run
- **THEN** the run writes nothing to the provider and the change ends `skipped` (`unverified`)

### Requirement: Where a trashed message goes back to is bound
When a run puts a message in the trash at Zoho and records where it was
(`restore_to`), the folder SHALL be one Zoho finds the message in when asked
just before the move (a message Zoho does not find there keeps no place),
and the run SHALL bind that place into the change's MAC, only while the
change still verifies as recorded. Before an untrash or a not-spam (out
of the trash or spam and into neither) uses a recorded place, the run SHALL
verify it; when it does not verify, the change SHALL NOT be made: it SHALL
end `skipped` (`unverified`), the message SHALL be re-read, and a WARN line
carrying ids and counts only SHALL be logged.

#### Scenario: A place edited with SQL
- **WHEN** a member trashes a Zoho message filed in one of their folders, SQL edits where it goes back to, and the member untrashes it
- **THEN** Zoho receives no move, the message stays in the trash and the untrash ends `skipped` (`unverified`)

#### Scenario: A place planted in the index
- **WHEN** SQL puts another folder in a Zoho message's indexed state before the member trashes it, and the member then untrashes it
- **THEN** Zoho does not find the message in that folder, no place is kept, and the received message goes to the Inbox folder, never to the planted one

#### Scenario: Trash and back
- **WHEN** nothing edits the place and the member untrashes the message
- **THEN** it goes back to the member's folder

### Requirement: What was bound before destinations were
The migration that started binding destinations SHALL record each `messages`
stream's inbox, connection and mailbox, and each outbound change's
`restore_to`, in tables the runtime role may read and never write; a stream
or a change removed takes its record with it. A send or change MAC of the
earlier encoding (`…/v1`) SHALL verify only while the destination resolved
now is the one recorded for its stream and that record's inbox is the one
the stream feeds now and the send's own, and a place of the earlier encoding
only while it is also the one recorded for its change. A MAC of the new
encoding SHALL never verify as one of the earlier.

#### Scenario: A change in flight at the deploy
- **WHEN** a change bound before the deploy is pending and its stream still goes where it went then
- **THEN** the run makes it

#### Scenario: A change in flight, repointed
- **WHEN** such a change's stream was pointed at another mailbox since the deploy, or the stream has no record
- **THEN** the run makes nothing and the change ends `skipped` (`unverified`)

#### Scenario: A send in flight, pointed at another inbox's stream
- **WHEN** a send released before the deploy is pointed by SQL at a stream the deploy recorded for another inbox (that stream made to claim the send's inbox, the send row naming it)
- **THEN** the send does not stand there: nothing is sent and it fails `unavailable`

#### Scenario: Writing the record
- **WHEN** a transaction of the runtime role inserts or updates a row of the record
- **THEN** it is refused

### Requirement: Finding a mailbox's servers
`connectors.discover_imap` SHALL find IMAP and SMTP settings for an email
address from, in order, the bundled provider database, the domain's own
autoconfig document (HTTPS only, through the host guard, no redirects), DNS
SRV records and the domain's MX host, without sending the address or its
domain to any third party. An address whose mail is hosted by a provider
Campbooks connects natively SHALL be answered with that provider instead of
IMAP settings. It SHALL be limited to 30 calls per member per hour.

#### Scenario: A big provider
- **WHEN** a member asks for `ana@yahoo.com`
- **THEN** the answer holds Yahoo's IMAP and SMTP servers from the bundled database and says an app password is needed

#### Scenario: A custom domain at a hosting company
- **WHEN** a member asks for `ana@bakery.example` and `autoconfig.bakery.example` publishes a configuration
- **THEN** the answer holds those servers with source `autoconfig`

#### Scenario: A custom domain on Google
- **WHEN** a member asks for an address whose domain's MX is at Google
- **THEN** the answer is `use_provider: google` and no IMAP settings

#### Scenario: Nothing found
- **WHEN** no source yields settings
- **THEN** the answer is `not_found` and the member can enter the servers by hand

### Requirement: Connecting an IMAP mailbox
`connectors.connect_imap` SHALL connect an IMAP mailbox only after signing
in to its IMAP server and authenticating to its SMTP server with the given
password, over TLS or STARTTLS with verified certificates, and SHALL keep
nothing when either fails. It SHALL require a member token, `connectors:write`,
permission to create a connection, and a sign-in within the last ten
minutes. Refusals SHALL name what went wrong with a stable code and SHALL
never repeat the server's own text. The connection SHALL carry the
`messages` capability and SHALL never be a way to sign in. A connection whose
address is not verified SHALL start its address check. Reconnecting a
connection whose address was verified, with the same address, SHALL keep it
verified.

#### Scenario: App password accepted
- **WHEN** a member who signed in five minutes ago connects `ana@yahoo.com` with a valid app password
- **THEN** the connection is created with provider `imap`, its `messages` stream starts, and `ConnectionAdded` is published

#### Scenario: Wrong password
- **WHEN** the IMAP server refuses the login
- **THEN** the answer is `imap_login_refused`, nothing is stored, and for Yahoo it adds that an app password is needed

#### Scenario: Server without TLS
- **WHEN** the IMAP server on port 143 does not offer STARTTLS
- **THEN** the answer is `starttls_missing` and no password was sent

#### Scenario: Old sign-in
- **WHEN** the member signed in more than ten minutes ago
- **THEN** the answer is 403 `step_up_required`

#### Scenario: Client token
- **WHEN** an API client token calls the operation
- **THEN** the answer is `connection_needs_user`

#### Scenario: Reconnecting with a new password
- **WHEN** the member who connected a mailbox reconnects it with a new valid password
- **THEN** the same connection keeps its streams, the new password replaces the old one, and its health is `ok`

#### Scenario: A username that is not the address
- **WHEN** a member connects `ana@bakery.example` signing in as `ana`
- **THEN** the connection's `address_verified` is false, its `address_check` is `checking`, and a check message is queued

#### Scenario: Turned off
- **WHEN** `CAMPBOOKS_CONNECTORS__IMAP__ENABLED` is false
- **THEN** both IMAP operations answer `provider_not_configured` and `imap` is not offered

### Requirement: Mail servers are reached safely
Every connection to a member-supplied host (IMAP, SMTP, autoconfig) SHALL
resolve the host, refuse it unless every address is public (not loopback,
private, link-local, CGNAT, metadata, documentation, multicast or their IPv6
forms), and connect to the address it checked. Only ports 993, 143, 465 and
587 SHALL be used. A self-hosted instance MAY allow private hosts and other
ports by setting.

#### Scenario: Metadata address
- **WHEN** a member enters an IMAP host that resolves to `169.254.169.254`
- **THEN** the answer is `host_refused` and no connection is opened

#### Scenario: Rebinding
- **WHEN** a host resolves to a public address at the check and a private one a moment later
- **THEN** Campbooks connects to the public address it checked

#### Scenario: Self-hosted mail server
- **WHEN** `CAMPBOOKS_CONNECTORS__IMAP__ALLOW_PRIVATE_HOSTS` is true and the host is `10.0.0.5`
- **THEN** the connection is attempted

### Requirement: Connecting cannot be used to guess passwords
Connect attempts SHALL be limited to 10 per member and 30 per workspace per
hour, and failed logins to 5 per target mailbox (its username on its IMAP
server, whatever address was typed) per hour across all workspaces, after
which attempts for that mailbox answer `too_many_attempts` for an hour.

#### Scenario: Guessing one mailbox
- **WHEN** five connect attempts for `bob@example.net` fail within an hour, from any workspaces
- **THEN** the sixth answers `too_many_attempts` without contacting the server

#### Scenario: Guessing one mailbox under another address
- **WHEN** five connect attempts signing in as `bob@example.net` have failed, and a sixth types `postmaster@example.org` with the username `bob@example.net` on the same IMAP server
- **THEN** it answers `too_many_attempts` without contacting the server

### Requirement: IMAP messages adapter
The IMAP adapter SHALL read a mailbox's Inbox, Sent, Junk, Trash, Archive
and user folders (never Drafts, `\All`, `\Flagged` or unselectable folders),
first the configured window (default 90 days) newest first, then each run
only the folders whose `STATUS` moved, and the window's flags again every
`CAMPBOOKS_CONNECTORS__IMAP__SWEEP_MINUTES` (default 360). Folder roles SHALL
come from SPECIAL-USE attributes, or from well-known folder names when the
server gives none. New messages SHALL be fetched with `BODY.PEEK[]` as raw MIME
and stored like Gmail's; reading SHALL never change a flag at the server.

#### Scenario: Backfill then incremental
- **WHEN** a new IMAP mailbox's stream runs until its backfill ends and a message then arrives in the Inbox
- **THEN** the window's messages are announced once each, and the next run announces only the new message

#### Scenario: Nothing moved
- **WHEN** a run finds every folder's `UIDNEXT`, `UIDVALIDITY` and message count unchanged and the server has no CONDSTORE
- **THEN** no folder is opened and nothing is announced

#### Scenario: Read in another app
- **WHEN** a message is marked read in another mail app on a server with CONDSTORE
- **THEN** the next run announces it as read without downloading it again

#### Scenario: Reading does not mark read
- **WHEN** the adapter downloads an unread message
- **THEN** the message is still unread at the server

#### Scenario: A server without SPECIAL-USE
- **WHEN** the server lists `INBOX.Sent Items` and `INBOX.Bulk Mail` without attributes
- **THEN** they are read as Sent and Junk

#### Scenario: Oversized message
- **WHEN** a message is larger than `CAMPBOOKS_CONNECTORS__IMAP__MAX_MESSAGE_BYTES`
- **THEN** it is recorded as oversized and not downloaded

#### Scenario: A folder that will not open
- **WHEN** the server refuses to open one of the mailbox's folders
- **THEN** the other folders are read, the run ends without another run at once, and that folder is tried again an hour later

### Requirement: An IMAP message is one message wherever it is
An IMAP message SHALL be identified by its normalised `Message-ID` (or a
digest of its headers when it has none), so that moving it between folders,
copying it, or a folder's UIDVALIDITY changing never creates a second
message. Its state SHALL be derived from all its locations, and each location
SHALL be kept for write-back.

#### Scenario: Moved in another app
- **WHEN** a message is moved from the Inbox to a user folder `Bakery` in another mail app
- **THEN** the same message is announced as archived with the label `Bakery`, and no new message appears

#### Scenario: UIDVALIDITY reset
- **WHEN** the server changes the Inbox's UIDVALIDITY
- **THEN** the Inbox is re-read, its messages are re-located by `Message-ID` without being downloaded or announced as new

#### Scenario: Deleted in another app
- **WHEN** a sweep finds a message in no tracked folder
- **THEN** it is announced as removed

#### Scenario: A reused Message-ID
- **WHEN** a message from someone else reuses the `Message-ID` of the member's own mail, with another `Date`, `From` or size
- **THEN** it is a message of its own, and the member's mail keeps its body

### Requirement: IMAP sessions are bounded and failures are classified
A sync run SHALL use one session, through the host guard and TLS of
`imap-connection`. Sessions SHALL be capped per IMAP host across all
processes, IDLE sessions included: a host that is not one of the big
providers' IMAP hosts Campbooks lists by name, matched exactly, SHALL see at
most
`CAMPBOOKS_CONNECTORS__IMAP__SESSIONS_PER_HOST` (default 4) sessions, and a
known provider's host at most
`CAMPBOOKS_CONNECTORS__IMAP__PROVIDER_SESSIONS_PER_HOST` (default 200). Of a
host's sessions, IDLE SHALL hold at most half; one workspace's runs SHALL
hold at most `CAMPBOOKS_CONNECTORS__IMAP__WORKSPACE_SESSIONS_PER_HOST`
(default 2); one workspace's IDLE sessions SHALL be at most
`CAMPBOOKS_CONNECTORS__IMAP__WORKSPACE_IDLE_SESSIONS_PER_HOST` (default 10)
and, on a host that is not a known provider, at most half its run share,
counted against it. One mailbox login SHALL hold at most one IDLE session.
Every answer of the server, the sign-in's included, SHALL be read within
bounds set before it is read, never sized by the server's own word. A login
refused in three consecutive sign-ins, by sync runs or IDLE sessions, SHALL
set the connection's health to `needs_reauth`; a server answering
`UNAVAILABLE`, `INUSE` or `LIMIT`, or refusing the connection, SHALL only
slow that stream down.

#### Scenario: Password changed at the provider
- **WHEN** the IMAP server refuses the stored password in three consecutive runs
- **THEN** the connection's health becomes `needs_reauth`, its streams pause, and `ConnectionNeedsReauth` is published once

#### Scenario: Busy server
- **WHEN** the server answers `[UNAVAILABLE]` at login
- **THEN** the stream is retried later and its health is not `needs_reauth`

#### Scenario: Many mailboxes on one provider
- **WHEN** five streams on the same IMAP host, not a known provider, are due at once
- **THEN** at most four run at the same time and the fifth runs at a later dispatch

#### Scenario: Many mailboxes on a known provider
- **WHEN** five streams on a known provider's IMAP host are due at once
- **THEN** all five run, each workspace within its share

#### Scenario: One workspace's share of a provider
- **WHEN** a workspace already holds two sessions to an IMAP host
- **THEN** its next stream on that host waits, and other workspaces' streams on it still run

#### Scenario: IDLE counts against a small host's sessions
- **WHEN** two IDLE sessions are open to a small host and two runs hold its other sessions
- **THEN** a third run on that host waits for the next dispatch

#### Scenario: One IDLE session per mailbox login
- **WHEN** the same mailbox login is connected in two workspaces and both may IDLE
- **THEN** one connection holds an IDLE session and the other is polled

#### Scenario: A workspace's IDLE share of a small host
- **WHEN** a workspace has two mailboxes on a small host whose run share is two
- **THEN** one of them is held in IDLE and the other is polled

#### Scenario: A huge answer to a sign-in
- **WHEN** the server answers a sign-in by announcing a literal larger than the sign-in allows
- **THEN** the sign-in is refused before any of it is read

### Requirement: IMAP push with IDLE
When `CAMPBOOKS_CONNECTORS__IMAP__ENABLED` and
`CAMPBOOKS_CONNECTORS__IMAP__IDLE_ENABLED` are both on (both default off),
each process that runs sync jobs SHALL hold at most
`CAMPBOOKS_CONNECTORS__IMAP__IDLE_SESSIONS` (default 50) IDLE sessions
(RFC 2177) on the Inbox of IMAP mailboxes, of which one workspace's
mailboxes SHALL hold at most
`CAMPBOOKS_CONNECTORS__IMAP__IDLE_SESSIONS_PER_WORKSPACE` (default 5), and
SHALL hold no database connection while a session waits. An `EXISTS`, `EXPUNGE`, `FETCH` or
`VANISHED` the server sends in IDLE SHALL ask the stream's schedule for a run
now, collapsing into a run already pending, at most once per
`CAMPBOOKS_CONNECTORS__IMAP__IDLE_DEBOUNCE_SECONDS` (default 30) per stream,
with the changes heard during that interval folded into one more ask at its
end. A session SHALL end IDLE (`DONE`), send `NOOP` and start IDLE again
every `CAMPBOOKS_CONNECTORS__IMAP__IDLE_REFRESH_MINUTES` (default 25, at
most 29).

#### Scenario: New mail during IDLE
- **WHEN** a message arrives in the Inbox of a mailbox whose session is in IDLE
- **THEN** exactly one run of that mailbox's stream is asked for, and that run announces the message

#### Scenario: A burst of new mail
- **WHEN** the server sends many `EXISTS` within one debounce interval
- **THEN** one run is asked for within the interval and one run is pending

#### Scenario: IDLE is renewed
- **WHEN** a session has been in IDLE for the refresh window
- **THEN** it sends `DONE`, `NOOP` and `IDLE` again on the same connection

#### Scenario: One workspace's share of a pool
- **WHEN** one workspace has more mailboxes that may IDLE than its share of a process's sessions, each on a host of its own
- **THEN** that process holds at most its share of them, and other workspaces' mailboxes still get sessions

#### Scenario: IDLE is off by default
- **WHEN** an instance reads IMAP mailboxes with `CAMPBOOKS_CONNECTORS__IMAP__IDLE_ENABLED` unset
- **THEN** no IDLE session is opened and every mailbox is polled

### Requirement: One IDLE holder per connection
An IMAP connection SHALL be held in IDLE by at most one process at a time,
through a lease renewed about every minute that expires after three
minutes. A session whose lease is lost, expired or withdrawn SHALL stop.
Every renewal and claim SHALL take a bounded time, and a process SHALL stop
every session it holds once two minutes have passed since it asked for its
last renewal that succeeded, whatever else it is doing. A process that
shuts down cleanly SHALL release its leases. A claim SHALL look at the
claimable leases longest unlooked-at first, so that leases passed over at
their caps never keep the others from being claimed.

#### Scenario: Two processes
- **WHEN** two processes claim IDLE sessions for the same connection
- **THEN** only one holds its lease and only one session signs in

#### Scenario: A lost lease
- **WHEN** a session's lease expires or is taken while it is in IDLE
- **THEN** the session stops at the holder's next renewal and another process may claim the connection

#### Scenario: A database that hangs
- **WHEN** every renewal of a process's leases hangs
- **THEN** its sessions stop two minutes after its last renewal that succeeded, before its leases can expire

#### Scenario: Mailboxes at their caps
- **WHEN** hundreds of mailboxes wait on a host whose IDLE sessions are all held, and one mailbox on another host may IDLE
- **THEN** the other mailbox is claimed within a few claims

#### Scenario: Shutdown
- **WHEN** a process holding IDLE sessions shuts down cleanly
- **THEN** its sessions end and its leases are released for another process to claim

### Requirement: Cutting access ends an IDLE session
An IDLE session SHALL end, with `DONE` and `LOGOUT`, within a renewal of
its lease once its connection needs the member, its stream is paused, its
password is replaced or its connection is removed. A connection that needs
the member, a paused stream, a replaced password and a removal SHALL
withdraw its lease in the same transaction; a session SHALL check at every refresh of IDLE, and
whenever it asks for a run, that its stream is not paused, its connection
does not need the member, its lease is still offered and still its own.

#### Scenario: Paused during IDLE
- **WHEN** a quiet mailbox's stream is paused while its session is in IDLE
- **THEN** its lease is withdrawn with the pause and the session ends at its holder's next renewal, before any refresh

#### Scenario: Needs the member during IDLE
- **WHEN** a sync run sets the connection to `needs_reauth` while its session is in IDLE
- **THEN** its lease is withdrawn and the session ends at its holder's next renewal

#### Scenario: A new password during IDLE
- **WHEN** the member connects the mailbox again with a new password while a session signed in with the old one is in IDLE
- **THEN** that session ends at its holder's next renewal, and the next run offers the mailbox again

### Requirement: IDLE sessions reconnect safely
An IDLE session that ends with an error SHALL reconnect after an
exponential backoff with jitter, and each reconnect SHALL resolve the host
through the kernel's guard, use the mail ports and TLS rules of
`imap-connection`, and open the sealed password again, keeping it no longer
than the sign-in. A password the server refuses SHALL count towards the
same three consecutive refusals as sync runs; a server answering
`UNAVAILABLE`, `INUSE` or `LIMIT` SHALL only back off. After six failures
in a row the lease SHALL be given up for a cooldown, the stream still
polled; a lease withdrawn and offered again SHALL keep its cooldown.

#### Scenario: A cooldown survives a withdrawal
- **WHEN** a given-up lease is withdrawn and offered again before its cooldown ends
- **THEN** no process claims it before the cooldown ends

#### Scenario: The host now resolves to a private address
- **WHEN** a session reconnects and the host's name now resolves to an address the guard refuses
- **THEN** nothing connects to it, and the lease is given up after six failures

#### Scenario: Password changed at the provider during IDLE
- **WHEN** two sync runs have had the password refused and a session's reconnect is refused too
- **THEN** the connection's health becomes `needs_reauth`, `ConnectionNeedsReauth` is published once, and the session ends

### Requirement: Polling stays beside IDLE
A stream SHALL be offered for IDLE only after a run whose server listed
`IDLE`, and only while it changed within
`CAMPBOOKS_CONNECTORS__IMAP__IDLE_QUIET_DAYS` (default 14); otherwise its
offer SHALL be withdrawn and the stream polled as before. While its lease is
held, a run SHALL set the stream's next poll no sooner than
`CAMPBOOKS_CONNECTORS__IMAP__IDLE_POLL_SECONDS` (default 900), a safety poll
that reads every folder.

#### Scenario: A server without IDLE
- **WHEN** a mailbox's server does not list `IDLE`
- **THEN** no session is claimed for it and its stream keeps its adaptive polling

#### Scenario: A quiet mailbox
- **WHEN** a mailbox held in IDLE has not changed for the quiet period
- **THEN** its next run withdraws the offer, its session stops, and it is polled

#### Scenario: Safety poll
- **WHEN** a run of a stream held in IDLE finds nothing new
- **THEN** the stream's next run is due no sooner than the IDLE poll interval

### Requirement: A server in IDLE is read within bounds
Everything an IMAP server sends during IDLE SHALL be read through the
session's bounded framer, at most 64 KiB a response, 1,000 responses and
1 MiB per IDLE period; past those the session SHALL be dropped and back off.
A session that gets no tagged answer within the command timeout of a
refresh SHALL be dropped. Logs SHALL carry ids, stable codes and counts,
never what the server said; a server's response code SHALL be logged only
when it is one of RFC 3501's or RFC 5530's, and as `other` otherwise.

#### Scenario: A server repeating EXISTS
- **WHEN** the server sends `EXISTS` without end during IDLE
- **THEN** the session is dropped past the cap, at most one run is asked for, and the connection backs off

### Requirement: IDLE leases are isolated by workspace
The lease table SHALL have row-level security enabled and forced: a
workspace's transaction SHALL see only that workspace's leases, the lease
claim SHALL work across workspaces only in the maintenance scope, through
read and update policies of its own, and SHALL change only a lease's
holder and times.

#### Scenario: Another workspace
- **WHEN** a transaction of another workspace, or one with no workspace, reads the leases
- **THEN** it sees none of the workspace's leases

### Requirement: Calendar access is offered
A member SHALL be able to add the `calendar` capability to a Google or Zoho
connection of theirs through the same front door and extend path as any
capability, so the connection gains it with no second connection. Google
calendar access SHALL ask for
`https://www.googleapis.com/auth/calendar.events` and
`https://www.googleapis.com/auth/calendar.calendarlist.readonly`; Zoho
calendar access SHALL ask for `ZohoCalendar.calendar.READ` and
`ZohoCalendar.event.ALL`, and SHALL be offered only while
`CAMPBOOKS_CONNECTORS__ZOHO_CALENDAR` is on (off by default). Signing in
SHALL keep asking for mail only. When the grant comes back, the module SHALL
list the account's calendars, publish `connectors.capability_granted` for
`calendar` with a blob listing them (provider id, name, colour, time zone,
primary, may be written, and whether the account owns it when the provider
says: Google's `accessRole` is `owner`, Zoho's `privilege` is `owner`), and
create a calendar stream due now. A provider that refuses the calendar API
(not enabled on the client's project) SHALL leave the connection's mail as it
was and mark the calendar stream's health with a reason a member can act on.

#### Scenario: Adding calendar to Gmail
- **WHEN** Inês adds calendar access to her Google connection
- **THEN** the consent asks for the two calendar scopes, the same connection gains `calendar`, `capability_granted` lists her calendars, and a calendar stream is due now

#### Scenario: Whose calendars they are
- **WHEN** Inês's Google calendar list has "Inês" (`accessRole` owner), "Marta" (writer) and "Holidays in Portugal" (reader)
- **THEN** the listed blob says "Inês" is owned and "Marta" and "Holidays in Portugal" are not, and "Inês" and "Marta" may be written

#### Scenario: Zoho calendar switched off
- **WHEN** `CAMPBOOKS_CONNECTORS__ZOHO_CALENDAR` is off and a member asks for calendar access on a Zoho connection
- **THEN** the answer is 409 `capability_not_offered` and nothing changes

#### Scenario: Calendar API not enabled
- **WHEN** Google answers 403 `accessNotConfigured` to the calendar list
- **THEN** mail keeps syncing and the calendar stream's health says the calendar API is not enabled for this server

### Requirement: Google Calendar adapter
The Google Calendar adapter SHALL read each calendar schedule asks it to
sync: first the events that end 90 days ago or later and every series still
recurring, with series returned once (not expanded into instances) and each
exception as its own event, recording the calendar's sync token; then changes
incrementally from that token. A token Google no longer accepts (410) SHALL
trigger a full read of that calendar that emits upserts for new or changed
events and removals for events it had that are gone. Each event SHALL be
normalised to the calendar change contract (provider id, etag, status,
start and end with time zone or all-day dates, recurrence rules, the series
and original start of an exception, title, place, description, organiser,
attendees and the member's response, and Campbooks' private properties
when present), with the text stored as a blob before the batch that
references it. Requests SHALL take from the provider's shared budget and
follow the run's backoff, throttling and health rules.

#### Scenario: First read then incremental
- **WHEN** a calendar stream runs for the first time, then again after Inês adds an event in Google
- **THEN** the first batch carries the calendar's events and series, and the second only the new event

#### Scenario: Expired token
- **WHEN** Google answers 410 to a sync token
- **THEN** the calendar is read in full, and events that disappeared are emitted as removals

### Requirement: Zoho Calendar adapter
While `CAMPBOOKS_CONNECTORS__ZOHO_CALENDAR` is on, the Zoho Calendar adapter
SHALL read each calendar schedule asks it to sync by date range in the
account's data centre, keep its own change marker in the stream's cursor,
and normalise events to the same calendar change contract. Events Campbooks
wrote SHALL be recognised by the provider id the module stored when it wrote
them, since Zoho keeps no private properties.

#### Scenario: A Zoho event
- **WHEN** a Zoho calendar holds "Accountant, Av. da Liberdade" on Thursday at 15:00
- **THEN** a batch carries it in the same shape as a Google event

### Requirement: The calendar change stream
The module SHALL publish `connectors.calendar_changed` v1 per calendar with
the batch's upserts and removals, ids and blob references only, in the
transaction that advances the calendar's cursor, so a batch is never lost or
applied twice. Batches for one calendar SHALL be published in order. A
calendar schedule does not sync SHALL not be read.

#### Scenario: Cursor and event together
- **WHEN** a run reads three changed events and the transaction commits
- **THEN** one `calendar_changed` batch with three upserts is published and the cursor moves; when it does not commit, neither happens

### Requirement: Outbound changes to calendars
The module SHALL consume `schedule.outbound_requested` v1 and, after the
schedule module confirms the request through the outbound-requests port,
record the change durably, bound to that request with a MAC as mail changes
are, before it is attempted; a change whose MAC does not verify SHALL never
be made. The calendar stream's next run SHALL make recorded changes before
it reads: create, update one event or one occurrence, delete, or the
member's response to an invitation. An update or delete SHALL send
`If-Match` with the etag the request names; a 412 SHALL end the change
`superseded` (`changed_at_provider`) and re-read the event, so the
provider's version reaches schedule. Events Campbooks creates in Google
SHALL carry private properties with the Campbooks event id, kind and source
link. Each change SHALL end `applied` (with the provider's new id and etag),
`superseded`, `skipped` or `failed` with a stable reason code and no event
text, and the outcome SHALL be published for schedule. Provider trouble
SHALL keep changes pending under the run's backoff, and a change that met
trouble eight times SHALL be `failed`.

#### Scenario: A move reaches Google
- **WHEN** Inês moves "Dentist" to 16:00 and the stream runs
- **THEN** Google's event moves with `If-Match` her etag, the change is `applied`, and schedule gets the new etag

#### Scenario: Changed at Google first
- **WHEN** the clinic moved the event in Google before the run
- **THEN** Google answers 412, the change is `superseded` (`changed_at_provider`), and the next batch carries Google's version

#### Scenario: A change planted with SQL
- **WHEN** a pending calendar change no member asked for is inserted in the database
- **THEN** the run writes nothing to the provider and the change ends `skipped` (`unverified`)

### Requirement: Calendar write-backs never echo
Changes that came from the provider SHALL never be written back. After a
write, the adapter SHALL hold the etag it got, so the provider's own report
of that write produces no change in the next batch. While a member's change
is pending, a run that emits that event SHALL emit it with the change
applied.

#### Scenario: After a write
- **WHEN** a member's create has been written to Google and the stream runs again
- **THEN** no `calendar_changed` batch carries that event unless Google changed it since

### Requirement: The consent asks for what the connection has
A consent that adds the `calendar` capability to a connection, or connects
one with it, SHALL ask the provider for the calendar scopes besides the
mailbox's, and a reconnect of a connection SHALL ask again for the scopes of
every capability that connection has, so the new grant loses none. A member
who leaves a calendar scope out at the provider's consent still connects their
mailbox: the connection then gains only the capabilities whose scopes were
granted.

#### Scenario: Adding calendar to a Google connection
- **WHEN** Inês asks for calendar access on her Google connection
- **THEN** the consent asks for the mailbox scope and the two calendar scopes, and the connection gains `calendar` when all are granted

#### Scenario: Reconnecting after the grant stopped working
- **WHEN** Inês reconnects a Google connection that has `calendar`
- **THEN** the consent asks for the calendar scopes again, so the connection keeps its calendar

#### Scenario: A calendar scope left out
- **WHEN** Inês grants the mailbox scope and one of the two calendar scopes
- **THEN** the connection gains `messages` only

### Requirement: Calendars are read when the member turns them on
The module SHALL consume `schedule.calendar_sync_changed` v1 and read a
provider calendar only while the member has it on. A calendar turned on again
SHALL be read from the start (its sync token is dropped); one turned off SHALL
not be read. Which calendar of the provider the event is about SHALL travel in
a `calendar-ref` v1 blob, since the provider's id can be an address and never
travels in an event. Deliveries can arrive in any order and more than once:
the newest event a calendar heard SHALL win, and an older or repeated one
SHALL change nothing.

#### Scenario: Turning a calendar on
- **WHEN** Inês turns on her "Holidays" calendar
- **THEN** its next read is a full one, and a batch for it is announced

#### Scenario: Turning a calendar off
- **WHEN** Inês turns it off again
- **THEN** the stream stops reading it

#### Scenario: Off and on, delivered out of order
- **WHEN** Inês turns "Holidays" off and straight back on, and the "on" is delivered first
- **THEN** the late "off" changes nothing, and the calendar is read from the start

### Requirement: A calendar stream says why it does not work
A calendar stream that cannot work SHALL carry a stable reason a member or an
operator can act on (`calendar_api_not_enabled`, `provider_errors`), shown on
the connection's stream, and SHALL leave the connection's health and its mail
streams as they are. Throttling and failures on the calendar stream SHALL slow
that stream only; a refused token or grant, which the mailbox shares, SHALL
follow the connection's rules. A reconnect clears the reason.

#### Scenario: Failures in a row
- **WHEN** the calendar stream fails as many times in a row as the connection's mail would take to be marked failing
- **THEN** the stream's reason is `provider_errors` and the connection's health stays `ok`

### Requirement: IDLE settings never stop a server that does not run IDLE
The server SHALL refuse to start on an IDLE safety poll
(`CAMPBOOKS_CONNECTORS__IMAP__IDLE_POLL_SECONDS`) of zero. It SHALL tie that
poll to the instance's poll bounds (`CAMPBOOKS_CONNECTORS__POLL__MIN_SECONDS`,
`…__MAX_SECONDS`) only when IMAP and IDLE are both enabled. A server with IDLE
off SHALL start whatever its poll bounds.

#### Scenario: Narrowed poll bounds with IDLE off
- **WHEN** an instance sets its poll bounds to 60–600 seconds and leaves IDLE off
- **THEN** the server starts

#### Scenario: Narrowed poll bounds with IDLE on
- **WHEN** the same instance turns IMAP and IDLE on with the default safety poll of 900 seconds
- **THEN** the server refuses to start, naming `CAMPBOOKS_CONNECTORS__IMAP__IDLE_POLL_SECONDS`

### Requirement: Outbound changes to IMAP
The module SHALL make members' changes to messages of an IMAP stream, with
the merge, supersede, `already_set`, `in_trash`, retry, expiry and give-up
rules of the Gmail write-back and in arrival order, in the stream's run
before it reads. Read and unread, starred and unstarred SHALL set or clear
`\Seen` and `\Flagged` on every copy of the message in a tracked folder
(`UID STORE … +FLAGS.SILENT` / `-FLAGS.SILENT`). Archive SHALL move the
copies in the Inbox or a stream folder to the Archive folder; unarchive
SHALL move the copies in Archive to the Inbox, or copy one from a member's
folder when none is in Archive; trash and spam SHALL move the copies in the
Inbox, a stream folder, Archive or the other bin to the Trash or Junk
folder, never the copies in the member's own folders or Sent, and the trash
change SHALL keep, bound into it, the folder each copy was in; untrash and
unspam SHALL move those copies back to that folder while the reader tracks
it, else to the Inbox for received mail and to Sent (else Archive) for the
member's own. A message SHALL count as in the trash (or spam) when a copy is
there and none is in the Inbox, a stream folder, Archive or the other bin.
A move SHALL use `UID MOVE` when the server offers MOVE, else, when it offers
UIDPLUS, `UID COPY`, then `UID STORE +FLAGS.SILENT (\Deleted)` and
`UID EXPUNGE` of only the UIDs the `COPYUID` answer names as copied (a
`COPYUID` with a UIDVALIDITY of zero, sets that do not pair up, or none at
all, names none: the originals are kept and the change is `failed`,
`refused`); it SHALL NOT be made at all on a server with neither, nor by
copy-and-expunge in a folder where `UID SEARCH DELETED` finds a message
outside the move already `\Deleted` (`skipped`, `unsupported_here`). An
expunge reported for more messages than were named SHALL stop the push. A
mark taken back after a failed expunge SHALL be taken off only the UIDs the
push marked. The module SHALL never send `EXPUNGE` without UIDs, `CLOSE`,
`DELETE` or `RENAME`, SHALL leave a written folder with `UNSELECT` where
offered, else `EXAMINE` of it, before any `STATUS`, SHALL never move a copy
into or out of a folder the reader does not track, and SHALL never empty the
Trash. A destination the reader does not track SHALL leave that part
unmade (`not_movable`). Label changes SHALL be recorded `skipped`
(`unsupported`). UIDs SHALL be sent as ranges, at most 500 a command.

#### Scenario: Read and starred round-trip without echo
- **WHEN** a member marks an IMAP message read and starred, the stream runs, then runs again
- **THEN** the server's copy is `\Seen` and `\Flagged`, the change is `applied`, and neither run announces the message

#### Scenario: Unread and unstarred clear every copy
- **WHEN** a member marks unread and unstars a message with a read, flagged copy in the Inbox and another in a member's folder
- **THEN** both copies lose `\Seen` and `\Flagged`

#### Scenario: Archive with MOVE
- **WHEN** a member archives a message in the Inbox of a server offering MOVE and UIDPLUS
- **THEN** the server receives `UID MOVE` to the Archive folder, the message's location is its new UID there, the change is `applied`, and the next run announces nothing for it

#### Scenario: Archive with UIDPLUS only
- **WHEN** a member archives a message on a server offering UIDPLUS and not MOVE
- **THEN** the server receives `UID COPY`, `UID STORE +FLAGS.SILENT (\Deleted)` and `UID EXPUNGE` of that UID only, and the message is in Archive only

#### Scenario: No MOVE and no UIDPLUS
- **WHEN** a member archives a message on a server offering neither MOVE nor UIDPLUS
- **THEN** the message stays in the Inbox, the change is `skipped` (`unsupported_here`), and the server never receives `EXPUNGE`, `COPY` or `\Deleted`

#### Scenario: Another app's deleted message survives
- **WHEN** a message in the Inbox is marked `\Deleted` in another app and a member archives another message of that Inbox on a server offering UIDPLUS only
- **THEN** no copy or expunge is sent, the other app's message is still in the Inbox, still `\Deleted`, and the archive is `skipped` (`unsupported_here`)

#### Scenario: A copy without COPYUID
- **WHEN** a server offering UIDPLUS answers a member's archive's `UID COPY` with `OK` and no `COPYUID`
- **THEN** no `\Deleted` and no `UID EXPUNGE` are sent, the message stays in the Inbox, and the archive is `failed` (`refused`)

#### Scenario: A COPYUID naming part of the copies
- **WHEN** members archive two Inbox messages in one run and the `COPYUID` names only the first
- **THEN** only the first is marked and expunged, the second stays in the Inbox unmarked, and its archive is `failed` (`refused`)

#### Scenario: More expunged than asked
- **WHEN** a server answers a `UID EXPUNGE` of one UID with two `EXPUNGE` responses
- **THEN** the push stops there and the run counts as failed

#### Scenario: Taking a mark back
- **WHEN** a `UID EXPUNGE` is refused after marking two messages, one of which the member had already marked `\Deleted`
- **THEN** `\Deleted` comes off the other one only, and both stay in the Inbox

#### Scenario: Trash
- **WHEN** a member puts a message with copies in the Inbox and a member's folder in the trash, then takes it out
- **THEN** only the Inbox copy moves to the Trash folder and back to the Inbox, the copy in the member's folder never moves, and no run announces the message

#### Scenario: Untrash received mail
- **WHEN** a member takes a received message out of the trash that Campbooks did not put there
- **THEN** its copy moves from Trash to the Inbox

#### Scenario: Unarchive the member's own reply
- **WHEN** a member unarchives a conversation whose only copy of a message is in Sent
- **THEN** the copy stays in Sent and the change is `skipped` (`not_movable`)

#### Scenario: A label change
- **WHEN** a member adds a label to an IMAP conversation
- **THEN** the change is recorded `skipped` (`unsupported`) and the server receives nothing

### Requirement: Streams as IMAP folders
An IMAP stream SHALL carry Primary in the Inbox and Updates, Receipts and
Newsletters in folders named `Campbooks/Updates`, `Campbooks/Receipts` and
`Campbooks/Newsletters` under the server's hierarchy delimiter (under the
Inbox when every folder of the server is), found by name (case-insensitive,
decoded) or created with `CREATE` and `SUBSCRIBE` by the push that first
needs one. A move to a stream SHALL move the message's copies in the Inbox or
a stream folder to that stream's folder. Each part of a folder name
Campbooks creates SHALL be encoded in modified UTF-7, be at most 64
characters, and be refused (`skipped`, `folder_name_refused`) when it is
empty, `.` or `..`, or holds the delimiter, `/`, `*`, `%`, or a control or
direction character; the whole name SHALL be at most 255 bytes. A mailbox
SHALL get at most `CAMPBOOKS_CONNECTORS__IMAP__MAX_CREATED_FOLDERS` (default
10) folders from Campbooks (`skipped`, `folder_cap` past it); no folder SHALL
ever be deleted or renamed, and a folder SHALL be created only once a move
the push makes puts a copy in it. The reader SHALL count a copy in a stream folder
as in the inbox and not as a label, and a received message's upsert SHALL
carry `provider_stream` when the server shows where it is: its stream
folder's stream (none for several), `primary` for a new message in the
Inbox or one moved there from a stream folder at the server; otherwise it
SHALL carry none.

#### Scenario: No folder for a move that moves nothing
- **WHEN** a member moves an archived IMAP conversation to Updates and the mailbox has no `Campbooks/Updates`
- **THEN** no folder is created and the change is `skipped` (`not_movable`)

#### Scenario: A stream folder created once and reused
- **WHEN** members move two IMAP conversations to Newsletters in two runs and the mailbox has no `Campbooks/Newsletters`
- **THEN** the server receives one `CREATE` of `Campbooks.Newsletters` and one `SUBSCRIBE`, both messages are moved into it, and no run announces them

#### Scenario: Back to Primary
- **WHEN** a member moves a conversation from Newsletters to Primary
- **THEN** its copy moves from `Campbooks/Newsletters` to the Inbox

#### Scenario: A hostile stream name
- **WHEN** a folder name is built for a stream named with the delimiter, `/`, `*`, `%`, a control character, 65 characters, `..`, or nothing
- **THEN** no name is built and nothing is created; a name with non-ASCII letters is encoded in modified UTF-7 as one part

#### Scenario: The folder creation cap
- **WHEN** Campbooks has created `CAMPBOOKS_CONNECTORS__IMAP__MAX_CREATED_FOLDERS` folders in a mailbox and a move needs another
- **THEN** no folder is created and the change is `skipped` (`folder_cap`)

#### Scenario: Sorted in another app
- **WHEN** a member moves a message from the Inbox to `Campbooks/Updates` in another mail app
- **THEN** the next run announces it in the inbox, without the folder as a label, with `provider_stream` `updates`

### Requirement: IMAP writes act only on what stands
An IMAP write SHALL act only on the locations recorded for the change's
stream, read in its workspace's transaction, for changes that passed the
standing check and their binding. Before writing a folder the module SHALL
open it with `SELECT` and compare its UIDVALIDITY with the locations': on a
mismatch it SHALL write none of that folder's copies and keep the change
pending for the next read to re-locate them. A UID the server no longer has
SHALL be a no-op: its location dropped, the message looked up by
`Message-ID` at the next complete run, and a change none of whose copies is
left `skipped` (`message_gone`). Where the server offers CONDSTORE, flag
writes SHALL be conditional on the folder's modseq as last read
(`UNCHANGEDSINCE`): a message changed at the server since SHALL keep the
server's state, the change `superseded` (`changed_at_provider`) and the
message emitted again. After a write, the locations (flags; a moved copy at
the UID `COPYUID` names, unless that UID is already recorded for another
message or named twice, when the read looks for the copy instead) and the
index SHALL hold what was written, in the
transaction that ends the change and before the run reads, so neither that
read nor a run an IDLE session asks for after hearing the write announces
it. A flag write SHALL be checked by reading the copies' flags back: one
the server answered `OK` to without showing it made (a flag unchanged, a
copy left out of the answer, a refused fetch) SHALL be `failed`
(`refused`), the message emitted again, and never retried. A change SHALL
meet trouble at most eight times before it is `failed` (`provider_errors`).

#### Scenario: UIDVALIDITY changed before the write
- **WHEN** the Inbox's UIDVALIDITY changes after the last read and a member then marks a message of it read
- **THEN** the server receives no `STORE` in that run, the read re-locates the message, and the next run marks it read at its new UID

#### Scenario: Moved elsewhere first
- **WHEN** a message is moved from the Inbox to a member's folder in another app, and before the next read a member archives it in Campbooks
- **THEN** no move is sent for it, the change is `skipped` (`message_gone`), and the next runs announce the message in the member's folder without error

#### Scenario: Changed at the server since the last read
- **WHEN** on a CONDSTORE server a message is marked read in another app and, before the next read, a member stars it in Campbooks
- **THEN** the server keeps its state, the change is `superseded` (`changed_at_provider`), and the message is announced as the server has it

#### Scenario: An IDLE echo of our own change
- **WHEN** the Inbox is held in IDLE and a member's archive of an Inbox message is written
- **THEN** the IDLE session asks for a run, and that run announces nothing

#### Scenario: Another workspace's changes
- **WHEN** a transaction of another workspace reads `connectors.outbound_changes` and `connectors.imap_locations`, or updates them
- **THEN** it sees none of this workspace's rows and changes none

#### Scenario: A server that does not apply what it accepts
- **WHEN** the server answers `OK` to a member's mark read without marking the message read
- **THEN** the change is `failed` (`refused`), the run announces the message unread as the server has it, and no further `STORE` is sent

#### Scenario: A COPYUID naming a taken UID
- **WHEN** a server's `COPYUID` names, as a moved copy's new UID, a UID already recorded for another message of the folder
- **THEN** the other message keeps its location and the moved message is located where the read finds it

#### Scenario: A flag the server will not show
- **WHEN** the server answers the read-back of a member's mark read without the message
- **THEN** the change is `failed` (`refused`), never `applied`

#### Scenario: The retry cap
- **WHEN** the server answers every write of a change `[UNAVAILABLE]` for eight runs
- **THEN** the change is `failed` (`provider_errors`) and no further write is attempted

### Requirement: IMAP writes share the run's session
IMAP writes SHALL be made over the stream's run's one session, after it
takes its host's session slots, so they count against the per-host,
per-workspace and per-login caps and never open a session beside the
login's IDLE session. Only a folder being written SHALL be opened with
`SELECT`; reading SHALL stay `EXAMINE` and `BODY.PEEK`. Every answer to a
write SHALL be read within the session's bounds; a busy answer SHALL stop
the push and slow the stream down, the unwritten changes staying pending.

#### Scenario: A write waits for a session slot
- **WHEN** a member marks a message read while the stream's host has no session slot free
- **THEN** no session is opened and no `STORE` sent, the change stays pending, and a later run makes it

#### Scenario: A write beside IDLE
- **WHEN** a member marks a message read while the mailbox's Inbox is held in IDLE
- **THEN** the run opens one session for the write and the read, and the IDLE session keeps its lease

#### Scenario: Busy while writing
- **WHEN** the server answers a write `[UNAVAILABLE]`
- **THEN** the change stays pending, the stream is throttled, and the next run makes it

### Requirement: Sending through an IMAP mailbox's SMTP server
A send from an inbox fed by an IMAP stream SHALL go through the connection's
SMTP submission server, at the address the kernel's guard resolved and checked
for that send, on a mail port only: TLS from the first byte when the server is
set to `tls` (465), or STARTTLS when it is set to `starttls` (587), the
certificate checked against the host name by the platform verifier. On a
`starttls` server that does not offer STARTTLS the module SHALL say nothing
more (no `AUTH`, no password, no `MAIL`) and the send SHALL fail
`unavailable`. It SHALL sign in with `AUTH PLAIN` or `AUTH LOGIN` and the
connection's sealed password, opened for that send only. A refused password
(a 5xx answer to `AUTH`) SHALL count towards the stream's shared login
refusals, the third in a row setting `needs_reauth` and failing the send
`needs_reauth`; a busy answer (4xx) SHALL be retried and never counted. A
send SHALL be made only while the connection's address is verified; otherwise
it SHALL fail `rejected` before any SMTP session. The envelope sender and the
`From` header SHALL be the connection's own address, as stored: a document
whose `From` address is any other SHALL not be sent (`rejected`).
The envelope recipients SHALL be the document's To, Cc and Bcc; the message
sent SHALL carry no `Bcc` header. A recipient the server refuses SHALL abort
the whole send before `DATA` (`rejected`). The message SHALL be built by the
outgoing-document MIME builder, so no line break in a subject or a display
name adds a header. Logs SHALL carry ids, reply codes and counts only.

#### Scenario: Sending over implicit TLS
- **WHEN** a member replies from an IMAP inbox whose SMTP server is set to `tls` on 465
- **THEN** the server receives the message over TLS with the document's `Message-ID`, `In-Reply-To` and `References`, and the send is `sent`

#### Scenario: Sending over STARTTLS
- **WHEN** the SMTP server is set to `starttls` on 587 and offers STARTTLS
- **THEN** the session is upgraded before `AUTH`, the message is delivered once and the send is `sent`

#### Scenario: No STARTTLS on 587
- **WHEN** the SMTP server set to `starttls` does not offer STARTTLS
- **THEN** no `AUTH` and no `MAIL` reach it, no password goes in the clear, and the send fails `unavailable`

#### Scenario: From another address
- **WHEN** a send's document is from an address that is not the connection's
- **THEN** no SMTP session is opened and the send fails `rejected`

#### Scenario: An address the login does not prove
- **WHEN** a connection signs in with a username that is not its address, and a member sends from it
- **THEN** no SMTP session is opened and the send fails `rejected`

#### Scenario: A certificate for another name
- **WHEN** the SMTP server's certificate is not valid for its host name
- **THEN** no `AUTH` and no `MAIL` reach it and the send fails `unavailable`

#### Scenario: Bcc in the envelope only
- **WHEN** a message has a Bcc recipient
- **THEN** the server receives `RCPT TO` for that recipient, the message it receives has no `Bcc` header, and the Sent copy keeps it

#### Scenario: A refused password
- **WHEN** the SMTP server answers `AUTH` with 535
- **THEN** the refusal is counted with the stream's login refusals, the send is retried later, and after the third refusal in a row the connection needs the member and the send fails `needs_reauth`

#### Scenario: No header injection
- **WHEN** a document's subject and a recipient's display name hold a line break followed by `Bcc: someone`
- **THEN** the message sent has no `Bcc` header and the server receives no `RCPT TO` for that address

### Requirement: An IMAP send goes out at most once
The module SHALL record an IMAP send's attempt, on its own, after the server
accepted `DATA` (354) and before the first byte of the message. A failure
before then (connecting, TLS, the greeting, EHLO, `AUTH`, `MAIL`, `RCPT`,
`DATA`), and a 4xx or 5xx answer to the message itself, SHALL be definite:
retried later on a 4xx or no answer, `rejected` on a 5xx (`too_large` on
552). A lost connection, a timeout or an unreadable answer once the message
started SHALL never be followed by another attempt: the send SHALL end
`unknown_outcome`, once the attempt has surely ended (no IMAP attempt starts
sooner than `CAMPBOOKS_CONNECTORS__IMAP__SEND_TIMEOUT_SECONDS` plus 30
seconds after the previous one was claimed). A runner that cannot claim the
attempt after 354 SHALL drop the connection without `QUIT` and without ending
the data. Every step of a submission SHALL have a time limit and end by the
submission's deadline, and at most 1 MiB SHALL be read from the server in one
session.

#### Scenario: Busy, then accepted
- **WHEN** the SMTP server answers the first `RCPT TO` with 451, then accepts the message on the next attempt
- **THEN** the message is delivered once and the send is `sent`

#### Scenario: Connection lost after the message
- **WHEN** the connection is lost after the message was sent and before the server's final answer
- **THEN** the server received one `DATA`, no second attempt is made, and the send fails `unknown_outcome`

#### Scenario: The attempt claimed by another runner
- **WHEN** another runner claims the attempt while this one waits for the server's 354
- **THEN** this one drops the connection: the server sees no end of data, receives no `QUIT` inside the message, and delivers nothing

#### Scenario: A server that never stops talking
- **WHEN** the SMTP server answers EHLO with more than 1 MiB and no end
- **THEN** the session ends within its bounds, nothing is sent, and the send is retried

### Requirement: IMAP send limits
Before connecting, an IMAP send SHALL take, from the shared budgets, one
message of its mailbox login's hourly allowance
(`CAMPBOOKS_CONNECTORS__IMAP__SEND_PER_HOUR`, default 50), its recipients
from the login's daily allowance (`…__SEND_RECIPIENTS_PER_DAY`, 500), one
message of its workspace's hourly allowance (`…__SEND_WORKSPACE_PER_HOUR`,
200) and, unless the SMTP host is one of a short list of big providers' that
limit each account themselves, one of its SMTP host's
(`…__SEND_HOST_PER_HOUR`, 1,000) of which one workspace SHALL take at most
half, the login's keyed by a digest of the login, never by the connection;
an empty budget SHALL make the send wait. What a send took SHALL be given
back only when nothing was sent and the server refused nothing. A message with more recipients than
`…__SEND_RECIPIENTS_PER_MESSAGE` (50) SHALL fail `rejected`; one larger than
`…__SEND_MAX_MESSAGE_BYTES` (25 MiB), or than the `SIZE` the server announces
in EHLO, SHALL fail `too_large` before `MAIL`. One SMTP session per mailbox
login SHALL be open at a time.

#### Scenario: Larger than the server takes
- **WHEN** the server announces `SIZE` smaller than the message
- **THEN** the server receives no `MAIL FROM` and no `DATA`, and the send fails `too_large`

#### Scenario: Over the hourly allowance
- **WHEN** a mailbox login has used its messages for the hour
- **THEN** its next send waits for the budget and the server receives nothing until then

#### Scenario: One workspace on a small host
- **WHEN** a workspace has used half of a small SMTP host's hourly allowance
- **THEN** its next send through that host waits, and other workspaces still send through it

#### Scenario: A refused recipient costs its budget
- **WHEN** the server refuses a send's recipient and the mailbox login had one message left for the hour
- **THEN** the send fails `rejected` and the login's next send waits for the budget

#### Scenario: Too many recipients
- **WHEN** a message names more recipients than `CAMPBOOKS_CONNECTORS__IMAP__SEND_RECIPIENTS_PER_MESSAGE`
- **THEN** no SMTP session is opened and the send fails `rejected`

### Requirement: The sent copy in an IMAP Sent folder
After an IMAP send is accepted, the job SHALL index the sent message under the
reader's key for its `Message-ID`, as a read copy in Sent that the mailbox
sent, and announce it, in the transaction that marks the send `sent`. Unless
its SMTP host is one of the providers known to keep a copy of what they send
(a short list in the code, matched exactly), the stream's next run SHALL,
once its read is done, look for the `Message-ID` in the Sent folder and, when
it is not there, `APPEND` the message (Bcc kept) with
`\Seen` to the SPECIAL-USE `\Sent` folder or the Sent folder the reader found
by name, as a synchronising literal within the server's `APPENDLIMIT` and
`CAMPBOOKS_CONNECTORS__IMAP__MAX_MESSAGE_BYTES`, through the session's
bounded framer, within the run's byte budget. Each copy's end SHALL be
recorded as it ends, in a transaction of its own, with the location
`APPENDUID` names, so neither the next read nor IDLE announces the message
again. A Sent folder that cannot be looked in SHALL get no copy. A busy
answer, a cut session or a Sent folder that will not open SHALL leave the
copy for the next run (three at most); any other refusal SHALL end it
`failed`. Nothing about the copy SHALL fail or repeat the send, or stop the
run's inbound mail. A sent message whose copy could not be made SHALL never
be removed for having no location.

#### Scenario: Appended with APPENDUID and no echo
- **WHEN** a send is accepted by a server offering UIDPLUS, and the stream then runs twice
- **THEN** Sent holds one `\Seen` copy appended by Campbooks, its location is recorded, and neither run announces the message

#### Scenario: A provider that keeps its own copy
- **WHEN** the SMTP host is on the list of providers that keep a copy
- **THEN** no `APPEND` is sent and the copy the server keeps is read as the sent message, without echo

#### Scenario: Already in Sent
- **WHEN** the server put the message in Sent itself before the run
- **THEN** no `APPEND` is sent and the message is not announced again

#### Scenario: An APPEND the server refuses
- **WHEN** the server answers the `APPEND` with `NO [OVERQUOTA]`
- **THEN** the send stays `sent`, the server receives no second message, the copy is `failed`, and the sent message stays in its conversation

#### Scenario: A copy that will not go
- **WHEN** the IMAP server hangs up on `APPEND` and new mail has arrived in the Inbox
- **THEN** the run reads the new mail, the copy waits for the next run, and the connection stays healthy

#### Scenario: A Sent folder that will not open
- **WHEN** the IMAP server refuses to open Sent
- **THEN** no `APPEND` is sent and the copy waits for the next run

#### Scenario: Within the run's byte budget
- **WHEN** two copies are pending and the first fills the run's byte budget
- **THEN** the run appends one, and the next run the other

#### Scenario: Larger than the server's APPENDLIMIT
- **WHEN** the server's `APPENDLIMIT` is smaller than the message
- **THEN** no `APPEND` is sent and the copy is `skipped`

#### Scenario: Copies are isolated by workspace
- **WHEN** a transaction of one workspace reads the sends' copies
- **THEN** it sees none of another workspace's

### Requirement: IMAP send settings never stop a server that does not read IMAP
The server SHALL check the IMAP send settings (`…__SEND_PER_HOUR`,
`…__SEND_RECIPIENTS_PER_MESSAGE`, `…__SEND_RECIPIENTS_PER_DAY`,
`…__SEND_WORKSPACE_PER_HOUR`, `…__SEND_HOST_PER_HOUR`,
`…__SEND_MAX_MESSAGE_BYTES`, `…__SEND_TIMEOUT_SECONDS`) at startup only while
`CAMPBOOKS_CONNECTORS__IMAP__ENABLED` is on, and SHALL tie none of them to the
poll bounds. Their defaults SHALL start with any valid poll bounds.

#### Scenario: A stray value with IMAP off
- **WHEN** an instance with IMAP off sets `CAMPBOOKS_CONNECTORS__IMAP__SEND_PER_HOUR` to 0
- **THEN** the server starts

#### Scenario: Production's poll bounds
- **WHEN** an instance sets its poll bounds to 60–120 seconds, with IMAP off or on and the send defaults
- **THEN** the server starts

### Requirement: Connecting a mailbox by its address
The Mailboxes page SHALL offer "Add a mailbox" (⇧C), a dialog that first asks
for the mailbox's address only and then asks `connectors.discover_imap` which
way it connects: a Gmail or Google Workspace address, or a Zoho one, SHALL be
offered its provider's sign-in (the front door, as Connect is) with "Also use
this account to sign in"; an Outlook, Hotmail, Live, MSN or Microsoft 365
address SHALL be offered Microsoft's sign-in the same way ("Connect Outlook")
when discovery answers `use_provider` with `microsoft` (the instance offers
Microsoft mail), and SHALL get a note that Outlook is coming soon and nothing
to connect when it answers `not_supported_yet`; any other address SHALL get
the app-password step. When discovery answers `provider_not_configured` the
dialog SHALL offer the instance's mailbox providers instead. The app-password
step SHALL show a short how-to for the provider, chosen from discovery's
`provider_hint` or the address's domain: Yahoo, AOL, iCloud, Fastmail, GMX
and web.de (each saying to turn on IMAP in its settings first), Yandex (also
saying a new app password can take a few hours to work), and a general text
for any other domain; each named guide SHALL link to the provider's own
public help page, opened in a new tab. It SHALL have one password field and
an "Advanced" disclosure, closed unless no settings were found, holding the
username and the IMAP and SMTP host, port and security, prefilled from
discovery, where only TLS or STARTTLS can be chosen. Connecting SHALL call
`connectors.connect_imap` and SHALL explain every refusal in plain words from
its stable `code` (never the problem's `title` or `detail`), with a general
message for any other; a refusal with `step_up_required` SHALL offer "Sign in
again", as the page does. The password SHALL never be logged, SHALL use
`autocomplete="new-password"`, and SHALL be cleared from the page's state
once the request is made, on success and when the dialog closes; it SHALL
never be held in a cached mutation. The dialog SHALL trap focus, close on
Escape with nothing kept, submit each step with Enter shown on its button,
fit 375px and desktop, and be in en, pt-PT, es-ES and fr.

#### Scenario: A Gmail address
- **WHEN** a member types `ana@gmail.com` and continues
- **THEN** the dialog offers "Connect Gmail" with "Also use this account to sign in", and pressing it starts `identity.start_link` with `connect` and `google`

#### Scenario: An Outlook address on an instance that offers Microsoft mail
- **WHEN** discovery answers `use_provider` with `microsoft`
- **THEN** the dialog offers "Connect Outlook" with "Also use this account to sign in", and pressing it starts `identity.start_link` with `connect` and `microsoft`

#### Scenario: An Outlook address
- **WHEN** discovery answers `not_supported_yet` with `microsoft`
- **THEN** the dialog says Outlook is coming soon and offers nothing to connect

#### Scenario: A Yahoo address
- **WHEN** discovery answers `settings` with `provider_hint` `yahoo.com` and `password` `app_password`
- **THEN** the dialog shows Yahoo's app-password steps with a link to Yahoo's help page, one password field, and Advanced closed holding Yahoo's servers

#### Scenario: A GMX address
- **WHEN** discovery answers `settings` for `ana@gmx.net`
- **THEN** the how-to says to turn on IMAP in GMX's settings first and links to GMX's page for it

#### Scenario: Nothing found
- **WHEN** discovery answers `not_found`
- **THEN** the app-password step opens with Advanced open, its username prefilled with the address

#### Scenario: A wrong password at a provider that needs an app password
- **WHEN** `connectors.connect_imap` answers `imap_login_refused` for an address discovery said needs an app password
- **THEN** the dialog says the provider needs an app password, the password field is empty, and nothing of the server's text is shown

#### Scenario: The sign-in is too old
- **WHEN** discovery or connecting answers 403 `step_up_required`
- **THEN** the dialog says to sign in again first, and its button signs in again and comes back to the page

#### Scenario: IMAP turned off on the instance
- **WHEN** discovery answers 409 `provider_not_configured`
- **THEN** the dialog offers the instance's mailbox providers' sign-in buttons

### Requirement: An IMAP mailbox's address is checked automatically
When an IMAP mailbox is connected (or connected again) and its address is not
verified (its username is not its address), the module SHALL check, without
the member typing anything, that mail to the address reaches that mailbox:
it SHALL send one message to the address, from the address, through the
connection's own SMTP server (the submission, guard, TLS, budgets and session
slot of IMAP sending), carrying a code of at least 128 bits derived from the
instance's key, the connection, a fresh nonce and the address, in its
`Message-ID`. The check SHALL keep the address it checks, and SHALL only
ever verify that address: connecting the mailbox again with its address not
verified SHALL end every check under way (`address_changed`), whatever the
daily budget, and verifying SHALL require, in its transaction, that the
connection's address is still the checked one. The
message SHALL be plain text, say it comes from Campbooks and why, be written
in the member's locale, and carry `Auto-Submitted: auto-generated`. The
stream's runs SHALL then look for it in that connection's Inbox only, over
the run's own session after its pending changes and before its read, with a
`UID SEARCH` of the header and a fetch of each candidate's `Message-ID`, and
SHALL count only a message whose `Message-ID` is exactly the expected one.
Found within 15 minutes of the send, the connection's `address_verified`
SHALL become true in the transaction that ends the check. Past 15 minutes
(whether or not a run looked) or after 30 searches, the check SHALL end
`failed`, a code found later SHALL verify nothing, and sending SHALL stay
refused. The check holds only while the member cannot read the code: the
module SHALL not claim more (the member holds the IMAP password and can add
messages to the Inbox). While a check waits, the stream SHALL run at most the poll's
minimum interval apart. The code SHALL never be logged, put in a job
payload, an event or a job error. Each connection SHALL have at most 5
checks a day.

#### Scenario: The code comes back
- **WHEN** a mailbox signed in as `ana` with the address `ana@bakery.example` is connected, and the check message arrives in its Inbox
- **THEN** the next run finds it, the connection's `address_verified` is true, and a send from that inbox goes out

#### Scenario: The code never comes
- **WHEN** the check message never reaches the Inbox
- **THEN** 15 minutes after it was sent the check is `failed`, the address stays unverified, and a send from it fails `rejected`

#### Scenario: A server that lies
- **WHEN** the IMAP server answers every `UID SEARCH` with UIDs whose messages do not carry the expected `Message-ID`
- **THEN** nothing is verified

#### Scenario: The code of another check
- **WHEN** the Inbox holds a check message of an earlier check of the same connection, or of another connection
- **THEN** it does not verify the address

#### Scenario: The address changed after the checks were spent
- **WHEN** a member read the code of a connection's fifth check of the day, then connects the same mailbox again under another address and files that message in its Inbox
- **THEN** the check is `failed` (`address_changed`) and the new address is not verified

#### Scenario: A code for one address
- **WHEN** a check's message for `ana@bakery.example` is found after the connection's address became `billing@bakery.example`
- **THEN** nothing is verified

#### Scenario: Found too late
- **WHEN** no run looked for 16 minutes after the send and the message is then in the Inbox
- **THEN** the check is `failed` (`not_found`) and the address is not verified

#### Scenario: The server will not send as the address
- **WHEN** the SMTP server answers the check's `MAIL FROM` or `RCPT TO` with 550
- **THEN** the check is `failed` and nothing more is sent

#### Scenario: Too many checks
- **WHEN** a connection's address has been checked 5 times today
- **THEN** no more check message is sent until the budget refills

#### Scenario: Checks are isolated by workspace
- **WHEN** a transaction of one workspace reads the address checks
- **THEN** it sees none of another workspace's

### Requirement: The check message is removed safely
Once found, the check message SHALL be removed from the Inbox by exactly its
UID, and only with the write paths that cannot remove anything else: with
UIDPLUS, opened read-write, its `Message-ID` checked again, no other message
of the Inbox marked `\Deleted`, then `UID STORE +FLAGS.SILENT (\Deleted)` and
`UID EXPUNGE` of that UID (the mark taken off again if the expunge fails);
else, with MOVE, a `UID MOVE` of that UID to the Trash folder the stream
reads; else it SHALL be left in place. An answer that reports more than one
message expunged SHALL end the run as a hostile server. The module SHALL
never send `EXPUNGE` without UIDs, `CLOSE`, `DELETE` or `RENAME` for it, and
SHALL leave the Inbox with `UNSELECT` or `EXAMINE`.

#### Scenario: Expunged with UIDPLUS
- **WHEN** the check message is found on a server with UIDPLUS and nothing else in the Inbox is marked deleted
- **THEN** only its UID is marked and expunged, and every other message stays

#### Scenario: Another message marked deleted
- **WHEN** another message of the Inbox is already marked `\Deleted` and the server offers no MOVE
- **THEN** nothing is expunged and the check message stays, and the address is still verified

#### Scenario: Neither UIDPLUS nor MOVE
- **WHEN** the server offers neither
- **THEN** the check message is left in the Inbox and the address is verified

### Requirement: Checking an address again
`POST /api/v1/w/{workspace_id}/connections/{connection_id}/address-check`
(`connectors.check_address`, scope `connectors:write`, access Write on the
connection) SHALL start a new check of an IMAP connection whose address is
not verified and whose last check ended or is older than 30 minutes, with a
new nonce and the caller's locale, and answer 202 with the connection. It
SHALL answer 409 `address_check_not_needed` for a connection that is not
IMAP or is verified, 409 `needs_reauth` for one that needs the member, 404
for one the caller may not see, and 429 `rate_limited` past the daily
checks. `ConnectionView` SHALL carry `address_check`: `checking` while a check
is sending or waiting and younger than 30 minutes, `failed` once it ended
unverified (or went stale), and null when the address is verified or was
never checked.

#### Scenario: Try again
- **WHEN** the grantor asks to check a failed address again
- **THEN** the answer is 202 with `address_check` `checking`, and a new check message is sent

#### Scenario: Nothing to check
- **WHEN** the connection's address is verified
- **THEN** the answer is 409 `address_check_not_needed`

### Requirement: IMAP is on by default
`CAMPBOOKS_CONNECTORS__IMAP__ENABLED` SHALL default to true and
`CAMPBOOKS_CONNECTORS__IMAP__IDLE_ENABLED` to false. A server whose settings
name no IMAP variable SHALL start with any valid poll bounds. Without the
instance secret key, IMAP SHALL not be offered and the server SHALL start
(with a warning), unless `CAMPBOOKS_CONNECTORS__IMAP__ENABLED` is set to true
explicitly, which SHALL stop it naming the missing key.

#### Scenario: Production's settings
- **WHEN** the server starts with `CAMPBOOKS_CONNECTORS__POLL__MIN_SECONDS=60`, `CAMPBOOKS_CONNECTORS__POLL__MAX_SECONDS=120`, the instance secret key and no IMAP variable
- **THEN** it starts, IMAP mailboxes can be connected, and IDLE is off

#### Scenario: No secret key
- **WHEN** an instance with no instance secret key and no IMAP variable starts
- **THEN** it starts and IMAP is not offered

#### Scenario: Asked for without a key
- **WHEN** an instance sets `CAMPBOOKS_CONNECTORS__IMAP__ENABLED=true` and has no instance secret key
- **THEN** it does not start, and the error names `CAMPBOOKS_SECRET_KEY`

### Requirement: Microsoft mail connections
The `microsoft` provider SHALL offer the `messages` capability only, through
the front door, with one consent for `offline_access`, `Mail.ReadWrite`,
`Mail.Send` and `User.Read` (and the identity scopes every round trip asks
for), at the tenant `CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT` (unset:
sign-in's `CAMPBOOKS_MICROSOFT__TENANT`, itself `common` by default, so
personal and work and school accounts both connect, and an app registered
for one tenant never widens), with the OAuth client Microsoft sign-in uses
(`CAMPBOOKS_MICROSOFT__CLIENT_ID` and its secret). When the grant arrives,
connectors SHALL refuse it unless every mail scope was granted (compared
without the `https://graph.microsoft.com/` prefix and ignoring case), then
read Graph `/me` with its access token: the mailbox key SHALL be the Graph
user id (1 to 64 characters of letters, digits and `-`) and the address
`mail`, else `userPrincipalName`, trimmed and lower-cased, which SHALL parse
as an address. The connection's account SHALL stay the ID token's `sub`. The
address SHALL count as verified only when the identity module verified the
ID token's address and it is that same address. Calls SHALL go only to
`login.microsoftonline.com` and `graph.microsoft.com`, follow no redirect,
time out after 15 seconds and refuse answers over 64 KiB. Errors SHALL never
carry a token, a code or an answer's body.

#### Scenario: Member connects Outlook
- **WHEN** a member starts a Microsoft connection for `messages` and the grant comes back through the intake with every mail scope
- **THEN** a connection with provider `microsoft`, the member as grantor and health `ok` is created, its messages stream's mailbox is the Graph user id and its address the mailbox's primary address, and `connectors.connection_added` and `connectors.capability_granted` are published

#### Scenario: The mail tenant follows sign-in's
- **WHEN** `CAMPBOOKS_MICROSOFT__TENANT` is one tenant id and `CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT` is unset
- **THEN** a mail consent and its token refreshes go to that tenant; set to `common`, they go to `common`

#### Scenario: Same account again
- **WHEN** the member connects the same Microsoft account again in the same workspace
- **THEN** the existing connection's credentials are replaced and no second connection or mailbox exists

#### Scenario: Reconnecting with another account
- **WHEN** a grant extending a Microsoft connection comes back for another Microsoft account
- **THEN** it is refused with `account_mismatch` and the connection is unchanged

#### Scenario: Another member's account
- **WHEN** another member of the workspace connects the same Microsoft account
- **THEN** it is refused with `account_connected_by_another_member`

#### Scenario: Mail access left out
- **WHEN** the grant lacks `Mail.ReadWrite`, `Mail.Send` or `User.Read`
- **THEN** it is refused with `consent_incomplete` and nothing is stored

#### Scenario: Graph answers something it should not
- **WHEN** Graph `/me` answers without an id, with an id that is not one, with no address, or with more than 64 KiB
- **THEN** the grant is refused and nothing is stored

#### Scenario: An unverified address
- **WHEN** the ID token's address was not verified, or Graph's address is another one
- **THEN** the connection is kept with `address_verified` false

#### Scenario: A sign-in's grant
- **WHEN** a person signed up with Microsoft while Microsoft mail is offered (the new account's second round trip asked for the mailbox), and adopts the held grant in their workspace
- **THEN** it becomes their Microsoft connection like a connect's grant

### Requirement: Microsoft connection health
A Microsoft connection's messages stream SHALL keep its health known: each
run SHALL get a valid access token (the stored one, else a refresh at the
mail tenant's token endpoint under the per-connection refresh lock) before it
calls Graph. A refresh that Microsoft answers with `invalid_grant`,
`interaction_required`, `consent_required` or `login_required`, or with an
AADSTS error saying the grant is gone (70000, 70008, 700082, 50173, 65001,
50076, 50079, 50057, 53003), SHALL make the connection `needs_reauth`
(`grant_revoked`) and pause its streams; 429 (or a 503 with `Retry-After`)
SHALL make it `throttled` until the given time, at most an hour, whether a
refresh, a read, a write or a send met it; Graph refusing the access token
SHALL drop it so the next run refreshes, and a grant refused run after run
SHALL need the member (`access_refused`). Disconnecting it SHALL delete its
tokens without calling Microsoft. On an instance that no longer offers
Microsoft mail, its runs SHALL ask nothing of Microsoft, its outbound changes
SHALL be skipped as `unsupported`, and a send through it SHALL end
`not_connected`.

#### Scenario: Grant revoked at Microsoft
- **WHEN** the refresh answers 400 `invalid_grant` with AADSTS70000
- **THEN** the connection's health becomes `needs_reauth` and its streams pause

#### Scenario: Throttled
- **WHEN** the refresh answers 429 with `Retry-After: 120`
- **THEN** the connection's health becomes `throttled` and the stream's next run is at least two minutes away

#### Scenario: Throttled while writing
- **WHEN** a patch answers 429 with `Retry-After: 120`
- **THEN** the connection is `throttled`, the change stays pending, and the next run, two minutes away, makes it

#### Scenario: A healthy grant
- **WHEN** the stored access token is still valid
- **THEN** the run reads the mailbox without refreshing it

### Requirement: Microsoft mail is off by default
`CAMPBOOKS_CONNECTORS__MICROSOFT__ENABLED` SHALL default to false. While it is
off, Microsoft SHALL not be offered as a mail connection (its mailbox scopes
are none, so the ways-in listing names no Microsoft mailbox, `start_link`
answers `mailbox_unsupported`, `start_connection` answers
`provider_not_configured`, and discovery answers `not_supported_yet`), even
where the Microsoft sign-in client is configured, no stream reads or writes
Microsoft, a Microsoft change is recorded `skipped` (`unsupported`) and one
still pending when it was turned off ends so at the stream's next run, a send
through a Microsoft connection ends `not_connected`, the notification
callback answers 404, and the server SHALL start whatever the other
Microsoft and connectors settings are, of any shape. While it is on,
Microsoft SHALL be offered only with the Microsoft client and the instance
secret key; either missing SHALL be a warning, never a stop, and so SHALL
notifications without a usable public URL. Only a tenant that is not
`common`, `organizations`, `consumers` or a tenant id, or a Microsoft
adapter setting out of range (`…__BACKFILL_DAYS` 1 to 3650, `…__MAX_MESSAGE_BYTES`
1 to 64 MiB, `…__RUN_BUDGET_BYTES` positive, `…__REQUESTS_PER_MINUTE` 1 to
100000, `…__MAX_FOLDERS` 1 to 1000, `…__NOTIFICATIONS` a boolean), with the
flag on, SHALL stop the server, naming the setting. Writing back and sending
SHALL add no setting. No Microsoft setting SHALL be tied to the poll bounds.

#### Scenario: Production's settings
- **WHEN** the server starts with `CAMPBOOKS_CONNECTORS__POLL__MIN_SECONDS=60`, `CAMPBOOKS_CONNECTORS__POLL__MAX_SECONDS=120`, the Microsoft sign-in client, and no Microsoft mail, subscription or callback variable
- **THEN** it starts, Microsoft is not offered as a mail connection, and no subscription is made

#### Scenario: Flag off with a broken tenant
- **WHEN** the flag is off and `CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT` is `not a tenant`
- **THEN** the server starts

#### Scenario: Flag off with broken settings
- **WHEN** the flag is off and `CAMPBOOKS_CONNECTORS__MICROSOFT__MAX_FOLDERS` is `lots` and `…__NOTIFICATIONS` is `maybe`
- **THEN** the server starts

#### Scenario: Flag off with a connected mailbox
- **WHEN** a Microsoft mailbox was connected and the flag is now off, and a member archives one of its messages and sends from it
- **THEN** the change is recorded `skipped` (`unsupported`), the send ends `not_connected`, and nothing is asked of Microsoft

#### Scenario: Flag turned off with a change waiting
- **WHEN** a Microsoft change is pending (waiting out throttling) and the flag is turned off
- **THEN** the stream's next run ends the change `skipped` (`unsupported`) instead of leaving it to expire, and nothing is asked of Microsoft

#### Scenario: Flag on without the client
- **WHEN** the flag is on and no Microsoft client is configured
- **THEN** the server starts with a warning and Microsoft is not offered

#### Scenario: Flag on with a broken tenant
- **WHEN** the flag is on and the tenant is `not a tenant`
- **THEN** the server refuses to start, naming `CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT`

#### Scenario: Flag on with a broken setting
- **WHEN** the flag is on and `…__MAX_FOLDERS` is `0`
- **THEN** the server refuses to start, naming that setting

### Requirement: Microsoft messages adapter
The Microsoft adapter SHALL read a mailbox's Inbox, Sent Items, Junk Email,
Deleted Items, Archive and the member's own folders through Microsoft Graph,
never Drafts, Outbox, Conversation History, Sync Issues, a folder under them
or a search folder. The well-known folders SHALL be found by Graph's names
and always read; the member's folders SHALL be listed by name, those already
read keeping their place ahead of new ones, up to
`CAMPBOOKS_CONNECTORS__MICROSOFT__MAX_FOLDERS` (default 100) in all. A folder
SHALL be taken for gone only when a listing saw every folder and not that
one; a folder the cap leaves out, or a listing cut short, SHALL never start a
pass, and a well-known folder Graph does not name once SHALL keep its role.
Each folder SHALL keep its own position in `/me/mailFolders/{id}/messages/delta`:
a first round over the window (`CAMPBOOKS_CONNECTORS__MICROSOFT__BACKFILL_DAYS`,
default 90, newest first), then only what changed, one page per folder per run and another
run at once while a folder has more. Messages SHALL be keyed by Graph's
immutable ids, so a move keeps the message. A new message SHALL be downloaded
as raw MIME from `/me/messages/{id}/$value` and stored like every adapter's,
recorded as oversized above `CAMPBOOKS_CONNECTORS__MICROSOFT__MAX_MESSAGE_BYTES`
(default 50 MiB, at most 64 MiB, four downloads at once) and with no
download started once a run has fetched
`CAMPBOOKS_CONNECTORS__MICROSOFT__RUN_BUDGET_BYTES` (default 64 MiB); a known
message whose folder, read state, flag or categories changed SHALL be
emitted again without a download. Drafts SHALL be skipped. A known message
removed from a folder's delta SHALL be looked up: gone is a removal, found
is a move. When Graph no longer has a folder's delta state (410,
`syncStateNotFound`, `resyncRequired`, `syncStateInvalid`) or a tracked
folder disappears, the stream SHALL read the window again in every folder
and then remove the indexed messages of the window that pass did not list.
A message SHALL be removed for not being listed only when the pass could
see it: never when its folder's first round listed as many messages as Graph
lists with a `$filter` (5000), nor when its folder is not read while the
last listing left folders out.
The upsert SHALL say in inbox for the Inbox and stream folders
(`Campbooks/<Stream>`, under any parent), spam for Junk Email, trash for
Deleted Items, read from `isRead`, starred for a flagged message, outbound
for Sent Items, its labels from the member's own folders and its Outlook
categories (by name), its provider stream from its stream folder or a
category named like a stream mark (Primary for unmarked mail in the Inbox),
its thread key from `conversationId`, and its headers from the raw message.
Reading SHALL never change, move or delete anything at Microsoft. URLs SHALL
be built from the fixed Graph base with ids as path segments; of Graph's
`@odata.nextLink` and `@odata.deltaLink` only the `$skiptoken` or
`$deltatoken` SHALL be kept.

#### Scenario: Backfill then a new message
- **WHEN** a Microsoft stream's mailbox has messages in its Inbox and Sent Items and one more arrives after the backfill
- **THEN** the backfill announces them all once, and the next run announces only the new one

#### Scenario: Delta pages
- **WHEN** a folder's delta answers with an `@odata.nextLink`
- **THEN** the run stores its `$skiptoken` and runs again at once, and the round ends at the page with an `@odata.deltaLink`, whose `$deltatoken` the next run starts from

#### Scenario: Read in Outlook
- **WHEN** a known message is marked read in Outlook
- **THEN** the next run announces it as read and does not download it again

#### Scenario: Moved to a folder
- **WHEN** a known message is moved from the Inbox to the member's folder "Clients"
- **THEN** the next run announces it out of the inbox with label "Clients", and does not download it again

#### Scenario: Deleted for good
- **WHEN** a known message is deleted at Microsoft
- **THEN** the next run announces its removal

#### Scenario: Delta state expired
- **WHEN** a folder's delta answers 410 or `syncStateNotFound`
- **THEN** the stream reads the window again in every folder, keeps what is still there without downloading it again, and removes what is gone

#### Scenario: Oversized message
- **WHEN** a new message's `$value` is larger than `CAMPBOOKS_CONNECTORS__MICROSOFT__MAX_MESSAGE_BYTES`
- **THEN** it is recorded as oversized and never downloaded again

#### Scenario: Drafts are not read
- **WHEN** the mailbox has a message in Drafts, and a search folder under the Inbox
- **THEN** no run lists or downloads the draft, and the search folder is never read

#### Scenario: The folder cap
- **WHEN** a mailbox at the folder cap gets a new folder whose name sorts first
- **THEN** the well-known folders and the folders read so far are still read, the new one is not, and no message is taken for gone

#### Scenario: Graph's listing limit
- **WHEN** a pass reads a folder whose first round lists 5000 messages, Graph's most
- **THEN** none of that folder's indexed messages the pass did not list is removed

#### Scenario: A well-known folder not named once
- **WHEN** Graph answers 404 once for the Archive's well-known name
- **THEN** the Archive keeps its role, no pass starts, and its messages stay

### Requirement: Microsoft throttling
Microsoft requests SHALL draw from a per-connection budget of
`CAMPBOOKS_CONNECTORS__MICROSOFT__REQUESTS_PER_MINUTE` (default 600), shared
by every process, subscription calls included. A Graph answer 429, or 503 with `Retry-After`, SHALL make
the connection `throttled` (`rate_limited`) and the stream wait the given
time, at most an hour; a 503 without it and other 5xx SHALL count as a
failed run with the usual backoff. Graph errors SHALL carry a status and a
sanitised code only, never a body, a token or a link.

#### Scenario: Throttled by Graph
- **WHEN** a folder's delta answers 429 with `Retry-After: 120`
- **THEN** the connection is `throttled` and the stream's next run is two minutes away

#### Scenario: An unreasonable Retry-After
- **WHEN** Graph answers 429 with `Retry-After: 86400`
- **THEN** the stream's next run is an hour away

### Requirement: Microsoft change notifications
With Microsoft mail offered, `CAMPBOOKS_CONNECTORS__MICROSOFT__NOTIFICATIONS`
true and an `https` `CAMPBOOKS_PUBLIC_URL` whose host is not `localhost`, a
Microsoft stream's run SHALL, before it reads, make sure its connection has a
Graph subscription to `me/messages` (`created,updated,deleted`) posting to
`<CAMPBOOKS_PUBLIC_URL>/hooks/microsoft/mail`: one that is missing SHALL be
created for about three days, one expiring within a day SHALL be renewed (a
renewal Graph no longer knows creates a new one). Each subscription SHALL
have its own random `clientState` of 256 bits, stored only as a keyed hash
under the instance key, with that key's fingerprint, and never logged; a
subscription whose hash was made under another key SHALL be made again with
a fresh `clientState`, so a key rotation never silently ends notifications. A failed creation SHALL be retried
no sooner than an hour later and never fail the run. While a subscription is
live the stream's next poll SHALL be `CAMPBOOKS_CONNECTORS__POLL__MAX_SECONDS`
away. Without notifications, or without such a URL, the stream SHALL be
polled adaptively, with a warning at start only when notifications were
asked for, and no subscription SHALL be made.

#### Scenario: Subscribed
- **WHEN** a Microsoft stream runs on an instance with notifications on
- **THEN** a subscription to the mailbox's messages exists for about three days and the stream's next poll is the longest poll interval away

#### Scenario: Renewal
- **WHEN** a run finds its subscription expiring within a day
- **THEN** it renews it at Graph and keeps the same subscription

#### Scenario: A key rotation
- **WHEN** a run finds its subscription's `clientState` hashed under an earlier instance key
- **THEN** it makes a new subscription with a fresh `clientState`, whose notifications verify

#### Scenario: No public callback
- **WHEN** notifications are on but `CAMPBOOKS_PUBLIC_URL` is `http://localhost:8080`
- **THEN** the server starts, no subscription is made, and the stream is polled adaptively

### Requirement: The Microsoft notification callback does the least
`POST /hooks/microsoft/mail` SHALL be public and outside the operation
catalogue, and SHALL be mounted only while Microsoft mail and notifications
are on (otherwise the path is the application's fallback). Before anything
else it SHALL be rate limited: at most 500 posts a second in each process
before the database is asked anything, then shared limits per client
address (1200 a minute) and for the whole instance (30000 a minute),
answering 429 over them. With a `validationToken` of at most 1024 characters it SHALL answer it
as `text/plain` and do nothing else. Otherwise it SHALL read at most 64 KiB of
JSON with at most 100 notifications, look their subscriptions up across
workspaces in the maintenance scope (`FOR SELECT` on
`connectors.microsoft_subscriptions` only), keep those whose `clientState`
verifies against the stored hash in constant time, and for each connection
kept, in its own workspace, make its stream's run due now unless the stream
is paused or waiting for the member (a pending run absorbs it). A
`subscriptionRemoved` lifecycle notification SHALL forget the subscription
and `reauthorizationRequired` SHALL make the run renew it. It SHALL answer
202 when it kept a notification and 404 otherwise, the same for an unknown
subscription and a wrong `clientState`. It SHALL call no provider and read
no mail.

#### Scenario: The validation handshake
- **WHEN** Graph posts with `validationToken=abc`
- **THEN** the answer is 200 `text/plain` `abc`

#### Scenario: A change
- **WHEN** Graph posts a notification for a known subscription with its `clientState`
- **THEN** the answer is 202 and that connection's stream has a run pending, and no other stream does

#### Scenario: A wrong clientState
- **WHEN** a notification names a known subscription with another `clientState`
- **THEN** the answer is 404 and no run is asked for

#### Scenario: An unknown subscription
- **WHEN** a notification names a subscription no connection has
- **THEN** the answer is 404

#### Scenario: Too large
- **WHEN** the body is larger than 64 KiB
- **THEN** it is refused without being read in full

#### Scenario: Over the limit
- **WHEN** the instance's limit is spent and a valid notification arrives
- **THEN** the answer is 429 and no subscription is looked up

#### Scenario: A paused stream
- **WHEN** a valid notification arrives for a connection that needs the member
- **THEN** no run is asked for

### Requirement: Microsoft subscriptions are isolated by workspace
`connectors.microsoft_subscriptions` SHALL hold one row per connection, its
connection and workspace, and its stream and connection, each pair a foreign
key, so a row can never pair one workspace's connection with another's
stream; it SHALL go with its connection. A workspace transaction SHALL see
only its own workspace's rows, a transaction with neither a workspace nor a
scope none, and the maintenance scope SHALL only read them.

#### Scenario: Another workspace
- **WHEN** a transaction set for another workspace reads the table
- **THEN** it sees no row

#### Scenario: The maintenance scope
- **WHEN** a maintenance-scope transaction updates or deletes a row
- **THEN** no row is changed

### Requirement: Outbound changes to Microsoft
A Microsoft stream's run SHALL make its pending, bound outbound changes
before it reads, through the shared plan, for at most 50 messages a run (the
rest in a run at once after it), their requests charged to the
connection's budget. `connectors.request_changes` SHALL record a Microsoft
change as the IMAP mapping does: read, flag, inbox, trash, spam and stream
marks; a label change SHALL be skipped (`unsupported`). At Graph, read SHALL
be `isRead` and starred `flag.flagStatus` (`PATCH /me/messages/{id}`);
archive SHALL move mail in the Inbox or a stream folder to Archive;
unarchive SHALL move mail in Archive or a member's folder to the Inbox;
trash SHALL move to Deleted Items, remembering the folder Graph says the
message was in, bound into the change; out of the trash SHALL move back to
that folder while it is read and not Junk Email or Deleted Items, else mail
the inbox received to the Inbox; spam SHALL move to Junk Email and not spam
mail the inbox received to the Inbox. Moves SHALL be
`POST /me/messages/{id}/move` to a folder id of the stream's cursor; a
folder the cursor does not have, or a message in a folder the reader does
not read, SHALL never be guessed: that part is not made (`not_movable`), the
rest is. Nothing SHALL ever delete a message permanently, empty a folder or
delete a folder: no `DELETE` and no `permanentDelete` SHALL be sent, and
only messages a pending, bound change names SHALL be written. After a write
the index SHALL hold the message's tokens as Graph now has it, so the next
read emits nothing for it. An emission SHALL show a member's change still
waiting as they left it, and a message whose change was refused, overtaken,
not movable or given up SHALL be emitted again with the provider's state.
A change whose patch was made but whose move has nowhere to go SHALL not end
without a word: the message SHALL be emitted again with Graph's state. A move
answered by a timeout or a 5xx may have gone through, so where a message
put in the trash was SHALL be recorded, bound into the change, before the
move is called: the run that finds the message already in Deleted Items ends
the change `applied` and keeps that place, so taking it out of the trash
still puts it back.

#### Scenario: Marked read and starred
- **WHEN** a member marks a Microsoft message read and stars it
- **THEN** Graph receives one patch with `isRead` true and one with `flag.flagStatus` `flagged`, both changes end `applied`, and the next run announces nothing for the message

#### Scenario: Archived
- **WHEN** a member archives a message in the Inbox
- **THEN** it is moved to the Archive folder, keeps its id, the index holds it in Archive, and the next run announces nothing for it

#### Scenario: Deleted and restored
- **WHEN** a member deletes a message in the member's folder "Clients", then takes it out of the trash
- **THEN** it is moved to Deleted Items, then back to "Clients", and no `DELETE` reaches Graph

#### Scenario: Spam
- **WHEN** a member reports a message in the Inbox as spam
- **THEN** it is moved to Junk Email

#### Scenario: No Archive folder
- **WHEN** a member archives a message and the mailbox has no Archive folder
- **THEN** nothing is moved and the change ends `skipped` (`not_movable`)

#### Scenario: A label change
- **WHEN** a member adds a label to a Microsoft message
- **THEN** the change is recorded `skipped` (`unsupported`) and nothing is written

#### Scenario: A patch whose move has nowhere to go
- **WHEN** a member marks a message read and archives it before the run, and the mailbox has no Archive folder
- **THEN** the message is marked read at Graph, nothing is moved, and the message is emitted again with Graph's state

#### Scenario: A trash move that went through unseen
- **WHEN** a member deletes a message in the member's folder "Clients", Graph moves it but the answer is lost, and the member takes it out of the trash after the next run
- **THEN** the delete ends `applied` without a second move, and the message is moved back to "Clients"

### Requirement: Streams in Outlook
A move of mail in the inbox (the Inbox or a stream folder) to a stream SHALL
put it where #316 reads that stream: for Primary, the Inbox without any
stream category; for another stream, the mailbox's `Campbooks/<Stream>`
folder (under any parent) when the reader has one, without any stream
category, else the Inbox with the category `Campbooks/<Stream>` in place of
any other stream category. The message's other categories SHALL be kept. A
move of mail out of the inbox SHALL not be made (`not_movable`). No folder
and no master category SHALL be created.

#### Scenario: A stream with its folder
- **WHEN** the mailbox has the folder `Inbox/Campbooks/Updates` and a member moves a message in the Inbox to Updates
- **THEN** it is moved to that folder and announced in the Updates stream after no echo

#### Scenario: A stream without a folder
- **WHEN** the mailbox has no Receipts folder and a member moves a message in the Updates folder to Receipts
- **THEN** it is moved to the Inbox with the category `Campbooks/Receipts`, keeping its other categories

#### Scenario: Back to Primary
- **WHEN** a member moves a message carrying the category `Campbooks/Newsletters` to Primary
- **THEN** the category is taken off and the message stays in the Inbox

### Requirement: Microsoft writes act only on what stands
Before it writes a message, the push SHALL read its state and `@odata.etag`
at Graph. When a token the change touches differs between the index and
Graph and Graph's is not what the member asks for, Graph changed it since
the last read: the change SHALL end `superseded` (`changed_at_provider`),
nothing SHALL be written, and the message SHALL be emitted again with
Graph's state. When Graph already shows what the member asks for, nothing
SHALL be called and the change SHALL count as made. A message Graph changed
in any way since the last read SHALL be emitted again with Graph's state
once written. A patch SHALL carry
`If-Match` with the etag read; a 412 or 409 SHALL re-read the message once
and apply the same rule again, and a second one SHALL end the change
`superseded`. A message with no etag SHALL not be patched. Only a 404 at the read SHALL mean
the message is gone (`skipped`, `message_gone`); any other answer that is
not the message (a 400, a 403, an answer that cannot be read) SHALL end the
change `failed` (`refused`) and the message SHALL be emitted again with
Graph's state. Messages SHALL be
read and moved with `Prefer: IdType="ImmutableId"`, so a move keeps the
index's row; a move whose answer names another id SHALL have the old id
looked up by the next read, which removes it once Graph no longer has it and
reads the new id as new.

#### Scenario: Changed between the read and the patch
- **WHEN** Outlook adds an unrelated category to a message after the push read it and before its patch
- **THEN** Graph answers 412, the push reads the message again, patches it with the new etag, and the change ends `applied`

#### Scenario: Graph's later change wins
- **WHEN** a member deletes a message that was moved to Junk Email in Outlook after the last read
- **THEN** nothing is moved, the change ends `superseded` (`changed_at_provider`), and the message is announced as spam

#### Scenario: Graph already did it
- **WHEN** a member marks a message read that was marked read in Outlook after the last read
- **THEN** nothing is patched and the change ends `applied`

#### Scenario: A read Graph refuses
- **WHEN** Graph answers 403 to the read of a message before a change
- **THEN** nothing is written, the change ends `failed` (`refused`), and the message is emitted again

#### Scenario: A move answering another id
- **WHEN** Graph answers a move with an id that is not the message's
- **THEN** the next read removes the old id and announces the new one once

### Requirement: Sending through Microsoft Graph
A send from a Microsoft inbox SHALL go out through `POST /me/sendMail` as the
RFC 5322 message the document describes (From the connection's address,
To, Cc, Bcc, Subject, Date, `Message-ID`, `In-Reply-To`, `References`, a
plain-text part, an HTML alternative when given, the attachments), base64,
with no `Sender`. It SHALL go out only when the connection's address is
verified and the document's From is that address, every recipient an
address; otherwise it SHALL fail `rejected` without calling Graph. A message
whose base64 is larger than 4 MB SHALL fail `too_large` without calling
Graph, and a 413 SHALL be `too_large`. Campbooks SHALL never write a copy to
Sent Items: after Graph accepts the message, the job SHALL look its copy up
in Sent Items by its `internetMessageId` and, when exactly one is there,
record the send sent with that copy's id and index and announce the message
under it, so the next sync emits nothing new. A message Graph accepted
(202) SHALL always end sent: when its copy is not found (not filed yet, filed
under another `Message-ID`, or a `Message-ID` that cannot be looked up), the
send SHALL be recorded sent under the message's own `Message-ID` as its
provider message id (as an IMAP send is), never `unknown_outcome`, and the
next sync announces the copy when it lists it.
Every
`sendMail` SHALL be preceded by an attempt
committed on its own; an unclear one (a timeout, a 5xx, a lost answer)
SHALL be looked up by its `internetMessageId` in Sent Items before anything
else: found, the send is sent; otherwise it SHALL never be sent again and
SHALL end `unknown_outcome`; this holds only for an attempt Graph did not
answer 202. 429, or 503 with `Retry-After`, SHALL make the connection
`throttled` (`rate_limited`), as for a read, and retry the send after that
wait, at most an hour.

#### Scenario: A reply
- **WHEN** a member replies from a Microsoft inbox
- **THEN** Graph receives one `sendMail` whose MIME carries the document's `Message-ID`, `In-Reply-To` and `References`, the send is sent with the Sent Items copy's id, and no copy is written to Sent Items

#### Scenario: A copy filed late
- **WHEN** Graph takes a `sendMail` but files its copy in Sent Items only after the job looked for it
- **THEN** the send is recorded sent under its own `Message-ID`, Graph receives no second `sendMail`, and the next sync announces the copy once

#### Scenario: A copy under another Message-ID
- **WHEN** Graph takes a `sendMail` and files a copy whose `Message-ID` is not the document's
- **THEN** the send is recorded sent under its own `Message-ID`, never `unknown_outcome`, and Graph receives no second `sendMail`

#### Scenario: The next sync
- **WHEN** the stream syncs after a send and Sent Items' delta lists the copy
- **THEN** no second upsert of that message is announced

#### Scenario: Too large
- **WHEN** a document's attachments make the base64 message larger than 4 MB
- **THEN** the send fails `too_large` and Graph receives no `sendMail`

#### Scenario: An unverified address
- **WHEN** a Microsoft connection's address is not verified
- **THEN** a send through it fails `rejected` and Graph receives no `sendMail`

#### Scenario: Another From
- **WHEN** a document's From is not the connection's address
- **THEN** the send fails `rejected` and Graph receives no `sendMail`

#### Scenario: A lost answer
- **WHEN** Graph takes a `sendMail` but its answer is lost, and the job runs again
- **THEN** the job finds the copy in Sent Items and Graph receives no second `sendMail`

#### Scenario: Throttled
- **WHEN** Graph answers a `sendMail` 429 with `Retry-After: 120`
- **THEN** the connection is `throttled`, the job tries again after two minutes, and the message goes out once

### Requirement: A connection says what it could still be given
The connection view (`connectors.get_connection`, `connectors.list_connections`
and the answers that return one) SHALL carry `offers`: the capabilities the
connection could be given by consenting again and does not hold yet. A Google
connection SHALL offer `calendar`; a Zoho connection SHALL offer `calendar`
only while `CAMPBOOKS_CONNECTORS__ZOHO_CALENDAR` is on and Zoho is configured;
Microsoft and IMAP connections SHALL offer nothing; a capability the
connection holds SHALL never be in it.

#### Scenario: Google offers calendar
- **WHEN** a Google connection holds `messages` only
- **THEN** its `offers` is `["calendar"]`, and once it holds `calendar` it is empty

#### Scenario: Zoho follows its flag
- **WHEN** a Zoho connection holds `messages` only
- **THEN** its `offers` is empty while `CAMPBOOKS_CONNECTORS__ZOHO_CALENDAR` is off and `["calendar"]` while it is on

#### Scenario: Microsoft and IMAP
- **WHEN** a Microsoft or an IMAP connection is read
- **THEN** its `offers` is empty

### Requirement: Connecting Notion
With `CAMPBOOKS_CONNECTORS__NOTION__ENABLED` on and a Notion client
configured, a member SHALL be able to connect their own Notion account with
the `pages` capability through an authorization-code flow owned by
connectors: a `state` bound to the browser, single use, valid for 10
minutes. The connection SHALL belong to that member, SHALL be identified by
Notion's `bot_id`, and SHALL keep its tokens sealed by the instance key and
bound to the connection row. Every refresh SHALL store the new access and
refresh tokens together, and an `invalid_grant` SHALL mark the connection as
needing a reconnect. A client token SHALL NOT start the flow. While the flag
is off, no Notion route or hook SHALL exist and no Notion setting SHALL stop
the server.

#### Scenario: Connecting
- **WHEN** Marta connects Notion and shares "Product roadmap" on Notion's consent screen
- **THEN** a Notion connection with the `pages` capability is stored as Marta's, and its tokens are readable only as that connection's

#### Scenario: A replayed callback
- **WHEN** a Notion callback arrives with a `state` that was already used
- **THEN** it is refused and no connection is stored

### Requirement: Notion changes reach Campbooks
Each linked Notion page SHALL have a change stream that reads the page's
blocks when the page is due, and emits a normalized snapshot: block ids,
kinds, attributes, rich-text runs, edit times and content hashes, with kinds
Campbooks lacks as `external`. Notion files SHALL be stored as blobs within
20 MiB and an allowed type. `POST /hooks/notion` SHALL exist whenever Notion
is on, before any webhook secret is configured. With a webhook secret set,
a post SHALL be accepted only with a valid `X-Notion-Signature` over its raw
body, SHALL only mark the matching streams due (for `page.content_updated`,
`page.properties_updated`, `page.moved`, `page.deleted` and `page.undeleted`;
any other event type SHALL be answered 200 and mark nothing), and SHALL never call Notion
itself; a verification-shaped body SHALL then be answered 404. With no
webhook secret set, the only body taken SHALL be exactly
`{"verification_token": <text of at most 200 characters>}`: it SHALL be kept
sealed with the instance key as the instance's one token (a newer one
replaces it), answered 200 and logged without its value; every other post
SHALL be answered 404. The token SHALL be readable only through the
instance-admin CLI (`campbooks-server admin notion-webhook-token`, with
`--clear` to forget it), never through an HTTP endpoint, an event or a log.
Without a webhook secret, connections SHALL poll. Calls SHALL go only to `api.notion.com` with `Notion-Version: 2026-03-11`, follow no
redirect, and stay within the connection's request budget.

#### Scenario: A forged webhook post
- **WHEN** a post to `/hooks/notion` carries a wrong signature
- **THEN** it is refused, and no stream is marked due

#### Scenario: Notion verifies the subscription
- **WHEN** no webhook secret is set and Notion posts `{"verification_token": "..."}` to `/hooks/notion`
- **THEN** the token is kept sealed, the post is answered 200, and the operator reads the token with the admin CLI

#### Scenario: A post that is not a verification token, before the secret is set
- **WHEN** no webhook secret is set and a post carries anything but a bare verification token
- **THEN** it is answered 404 and nothing is kept

#### Scenario: A verification token after the secret is set
- **WHEN** a webhook secret is set and a verification-shaped body is posted
- **THEN** it is answered 404 and no token is kept

### Requirement: Campbooks changes reach Notion
Notion block updates, appends and deletes SHALL be made only from outbound
changes bound by a MAC to their workspace, stream, Notion page, block and
kind, and verified before the call. A block whose Notion edit time moved
since the recorded base SHALL NOT be written. A Notion page SHALL never be
archived or deleted by Campbooks.

#### Scenario: Notion changed it first
- **WHEN** Campbooks is about to update a block that Rui edited in Notion after the last sync
- **THEN** the update is not sent, and the next inbound run raises a conflict for that block

### Requirement: Notion databases reach Campbooks
With Notion databases enabled, each linked Notion database SHALL have a
change stream. It reads the schema when it changed, and reads the entries
edited since its cursor. It SHALL emit normalised, hashed values, and SHALL
stay within the connection's request budget. The webhook events
`data_source.content_updated` and `data_source.schema_updated` SHALL only
mark that database's streams due.

#### Scenario: A schema change in Notion
- **WHEN** Rui adds a column "Region" to the Notion database "Clients"
- **THEN** the next run reads the new schema and emits it, and nothing is written to Notion

### Requirement: Campbooks changes reach Notion databases
Schema changes, new rows, cell updates and row trashing SHALL be made in
Notion only from MAC-bound outbound changes verified before the call. A
change whose base edit time or schema hash moved SHALL be held back. Removing
a Notion column SHALL be made only from a change that carries the member's
confirmation. A Notion database SHALL never be deleted.

#### Scenario: A column removal without confirmation
- **WHEN** an outbound change to remove a Notion column lacks the confirmation
- **THEN** it is not sent, and nothing changes in Notion

### Requirement: Listing shared Notion databases
`connectors.list_notion_data_sources` SHALL answer the Notion databases the
caller's own Notion connection can see (id, title, how many columns it has,
and whether it is linked to a table already), 100 at a time with a cursor and
at most 200 in all, within the connection's request budget. It SHALL answer
404 for a connection that is not the caller's, and an empty list, without
calling Notion, when Notion databases are off.

#### Scenario: Another member's connection
- **WHEN** Rui asks for the databases of Marta's Notion connection
- **THEN** the response is 404 and Notion is not called

#### Scenario: Notion databases are off
- **WHEN** Marta asks for the databases of her own Notion connection and Notion databases are off on the instance
- **THEN** the answer is an empty list and Notion is not called
