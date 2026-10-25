## ADDED Requirements

### Requirement: Microsoft messages adapter
The Microsoft adapter SHALL read a mailbox's Inbox, Sent Items, Junk Email,
Deleted Items, Archive and the member's own folders through Microsoft Graph,
never Drafts, Outbox, Conversation History, Sync Issues or a folder under
them, at most `CAMPBOOKS_CONNECTORS__MICROSOFT__MAX_FOLDERS` (default 100).
Each folder SHALL keep its own position in `/me/mailFolders/{id}/messages/delta`:
a first round over the window (`CAMPBOOKS_CONNECTORS__MICROSOFT__BACKFILL_DAYS`,
default 90), then only what changed, one page per folder per run and another
run at once while a folder has more. Messages SHALL be keyed by Graph's
immutable ids, so a move keeps the message. A new message SHALL be downloaded
as raw MIME from `/me/messages/{id}/$value` and stored like every adapter's,
recorded as oversized above `CAMPBOOKS_CONNECTORS__MICROSOFT__MAX_MESSAGE_BYTES`
(default 50 MiB) and with no download started once a run has fetched
`CAMPBOOKS_CONNECTORS__MICROSOFT__RUN_BUDGET_BYTES` (default 64 MiB); a known
message whose folder, read state, flag or categories changed SHALL be
emitted again without a download. Drafts SHALL be skipped. A known message
removed from a folder's delta SHALL be looked up: gone is a removal, found
is a move. When Graph no longer has a folder's delta state (410,
`syncStateNotFound`, `resyncRequired`, `syncStateInvalid`) or a tracked
folder disappears, the stream SHALL read the window again in every folder
and then remove the indexed messages of the window that pass did not list.
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
- **WHEN** the mailbox has a message in Drafts
- **THEN** no run lists or downloads it

### Requirement: Microsoft throttling
Microsoft requests SHALL draw from a per-connection budget of
`CAMPBOOKS_CONNECTORS__MICROSOFT__REQUESTS_PER_MINUTE` (default 600), shared
by every process. A Graph answer 429, or 503 with `Retry-After`, SHALL make
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
under the instance key and never logged. A failed creation SHALL be retried
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

#### Scenario: No public callback
- **WHEN** notifications are on but `CAMPBOOKS_PUBLIC_URL` is `http://localhost:8080`
- **THEN** the server starts, no subscription is made, and the stream is polled adaptively

### Requirement: The Microsoft notification callback does the least
`POST /hooks/microsoft/mail` SHALL be public and outside the operation
catalogue, and SHALL answer 404 unless Microsoft mail and notifications are
on. With a `validationToken` of at most 1024 characters it SHALL answer it
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
SHALL make it `throttled` until the given time; Graph refusing the access
token SHALL drop it so the next run refreshes, and a grant refused run after
run SHALL need the member (`access_refused`). Its outbound changes SHALL be
skipped as `unsupported`, a send through it SHALL end `not_connected`, and
disconnecting it SHALL delete its tokens without calling Microsoft. On an
instance that no longer offers Microsoft mail, its runs SHALL ask nothing of
Microsoft.

#### Scenario: Grant revoked at Microsoft
- **WHEN** the refresh answers 400 `invalid_grant` with AADSTS70000
- **THEN** the connection's health becomes `needs_reauth` and its streams pause

#### Scenario: Throttled
- **WHEN** the refresh answers 429 with `Retry-After: 120`
- **THEN** the connection's health becomes `throttled` and the stream's next run is at least two minutes away

#### Scenario: A healthy grant
- **WHEN** the stored access token is still valid
- **THEN** the run reads the mailbox without refreshing it

### Requirement: Microsoft mail is off by default
`CAMPBOOKS_CONNECTORS__MICROSOFT__ENABLED` SHALL default to false. While it is
off, Microsoft SHALL not be offered as a mail connection (its mailbox scopes
are none, so the ways-in listing names no Microsoft mailbox, `start_link`
answers `mailbox_unsupported`, `start_connection` answers
`provider_not_configured`, and discovery answers `not_supported_yet`), even
where the Microsoft sign-in client is configured, no stream reads Microsoft,
the notification callback answers 404, and the server SHALL start whatever
the other Microsoft and connectors settings are, of any shape. While it is
on, Microsoft SHALL be offered only with the Microsoft client and the
instance secret key; either missing SHALL be a warning, never a stop, and so
SHALL notifications without a usable public URL. Only a tenant that is not
`common`, `organizations`, `consumers` or a tenant id, or a Microsoft
adapter setting out of range (`…__BACKFILL_DAYS` 1 to 3650, `…__MAX_MESSAGE_BYTES`
1 to 1 GiB, `…__RUN_BUDGET_BYTES` positive, `…__REQUESTS_PER_MINUTE` 1 to
100000, `…__MAX_FOLDERS` 1 to 1000, `…__NOTIFICATIONS` a boolean), with the
flag on, SHALL stop the server, naming the setting. No Microsoft setting
SHALL be tied to the poll bounds.

#### Scenario: Production's settings
- **WHEN** the server starts with `CAMPBOOKS_CONNECTORS__POLL__MIN_SECONDS=60`, `CAMPBOOKS_CONNECTORS__POLL__MAX_SECONDS=120`, the Microsoft sign-in client, and no Microsoft mail, subscription or callback variable
- **THEN** it starts, Microsoft is not offered as a mail connection, and no subscription is made

#### Scenario: Flag off with a broken tenant
- **WHEN** the flag is off and `CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT` is `not a tenant`
- **THEN** the server starts

#### Scenario: Flag off with broken settings
- **WHEN** the flag is off and `CAMPBOOKS_CONNECTORS__MICROSOFT__MAX_FOLDERS` is `lots` and `…__NOTIFICATIONS` is `maybe`
- **THEN** the server starts

#### Scenario: Flag on without the client
- **WHEN** the flag is on and no Microsoft client is configured
- **THEN** the server starts with a warning and Microsoft is not offered

#### Scenario: Flag on with a broken tenant
- **WHEN** the flag is on and the tenant is `not a tenant`
- **THEN** the server refuses to start, naming `CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT`

#### Scenario: Flag on with a broken setting
- **WHEN** the flag is on and `…__MAX_FOLDERS` is `0`
- **THEN** the server refuses to start, naming that setting
