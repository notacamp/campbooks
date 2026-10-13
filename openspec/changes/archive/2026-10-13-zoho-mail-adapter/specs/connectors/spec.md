## ADDED Requirements

### Requirement: Zoho Mail connections in their data centre
The module SHALL connect Zoho Mail accounts when `CAMPBOOKS_ZOHO__CLIENT_ID`
and a client secret are set, asking in the grant for
`ZohoMail.accounts.READ`, `ZohoMail.folders.ALL` and `ZohoMail.messages.ALL`.
A Zoho grant SHALL be taken only when its issuer is one of Zoho's accounts
servers (`.com`, `.eu`, `.in`, `.com.au`, `.jp`, `.ca`, `.sa`, `.uk`), or, in
development only, the server `CAMPBOOKS_ZOHO__BASE_URL` names; any other
SHALL be refused with `unknown_data_centre`. Its tokens SHALL be refreshed
and revoked at that accounts server, and its mail read and written at that
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
Metadata SHALL be HTML-decoded before use. The upsert SHALL say in inbox
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
for an archived message, a move to the Inbox folder for one in another
folder). Label changes and moves between streams SHALL be recorded `skipped`
(`label_not_at_provider`, `no_provider_equivalent`). After a write, the
index SHALL hold the state written, so the next run emits nothing for it.

#### Scenario: Archive reaches Zoho
- **WHEN** a member archives a conversation whose message is in the Zoho Inbox
- **THEN** Zoho receives `archiveMails` for it, the change is `applied`, and the next run emits no batch for that message

#### Scenario: Unarchive a moved message
- **WHEN** a member unarchives a message that sits in another Zoho folder
- **THEN** Zoho receives `moveMessage` to the Inbox folder and the change is `applied`

### Requirement: Sending through Zoho
A send from an inbox fed by a Zoho stream SHALL go through Zoho's send API
from the stream's mailbox, with To, Cc, Bcc, subject, the HTML alternative
(or the text) and the attachments, uploaded first. A reply whose
`In-Reply-To` names a message of the stream SHALL be sent as Zoho's reply to
that message. The idempotency rules of Gmail sending SHALL hold, with one
difference: while a send is unclear, the next attempt SHALL look in the
mailbox's Sent folder for a message sent since that attempt with the same
subject and recipients, not already another send's, and record it as sent
if found. Once Zoho accepts, the sent message SHALL be fetched, indexed and
announced in the job's transaction like Gmail's; when it cannot be fetched
the send SHALL still be `sent` and the stream due at once.

#### Scenario: A reply goes out as a Zoho reply
- **WHEN** a member replies to a message of a Zoho inbox
- **THEN** Zoho receives one reply call naming that message, and the sent copy joins the same conversation

#### Scenario: A timeout after Zoho accepted the message
- **WHEN** Zoho accepts a message but the answer is lost, and the job runs again
- **THEN** the job finds it in the Sent folder and Zoho receives no second send

### Requirement: Zoho request budget
Requests to Zoho SHALL draw from a budget per Zoho login,
`CAMPBOOKS_CONNECTORS__ZOHO__REQUESTS_PER_MINUTE` (default 60), shared by
every process; a run SHALL take its listings and a share of downloads
before it starts and wait for the budget otherwise. Zoho's 429, or an answer coded as too many
requests, SHALL throttle only that stream until `Retry-After`.

#### Scenario: One login's backfill does not spend another's
- **WHEN** one Zoho login's budget is spent and another login's stream is due
- **THEN** the second stream runs at once

## MODIFIED Requirements

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
