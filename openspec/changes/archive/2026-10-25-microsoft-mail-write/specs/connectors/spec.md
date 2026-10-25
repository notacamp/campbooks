## ADDED Requirements

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
`superseded`. A message with no etag SHALL not be patched. Messages SHALL be
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
under it, so the next sync emits nothing new; while the copy is not there,
the attempt SHALL stay unclear (looked up again, never sent again). Every
`sendMail` SHALL be preceded by an attempt
committed on its own; an unclear one (a timeout, a 5xx, a lost answer)
SHALL be looked up by its `internetMessageId` in Sent Items before anything
else: found, the send is sent; otherwise it SHALL never be sent again and
SHALL end `unknown_outcome`. 429, or 503 with `Retry-After`, SHALL retry the
send after that wait, at most an hour.

#### Scenario: A reply
- **WHEN** a member replies from a Microsoft inbox
- **THEN** Graph receives one `sendMail` whose MIME carries the document's `Message-ID`, `In-Reply-To` and `References`, the send is sent with the Sent Items copy's id, and no copy is written to Sent Items

#### Scenario: A copy filed late
- **WHEN** Graph takes a `sendMail` but files its copy in Sent Items only after the job looked for it
- **THEN** the send is not settled yet, the next attempt finds the copy and records the send sent, and Graph receives no second `sendMail`

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
- **THEN** the job tries again after two minutes and the message goes out once

## MODIFIED Requirements

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
Microsoft, a Microsoft change is recorded `skipped` (`unsupported`), a send
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

#### Scenario: Flag on without the client
- **WHEN** the flag is on and no Microsoft client is configured
- **THEN** the server starts with a warning and Microsoft is not offered

#### Scenario: Flag on with a broken tenant
- **WHEN** the flag is on and the tenant is `not a tenant`
- **THEN** the server refuses to start, naming `CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT`

#### Scenario: Flag on with a broken setting
- **WHEN** the flag is on and `…__MAX_FOLDERS` is `0`
- **THEN** the server refuses to start, naming that setting
