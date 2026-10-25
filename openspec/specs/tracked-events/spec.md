# tracked-events Specification

## Purpose
The record of every action (ADR 0020): who did it, in what context, and what
the records it touched looked like then. Defined by the tracking plan
workbook, written in the transaction of the change, and carrying no personal
data, so the subscribers built on it (the warehouse, the admin event view)
read one complete record.

## Requirements

### Requirement: The tracking plan is the workbook
The system SHALL define every tracked event in `tracking/tracking-plan.xlsx`
(sheets README, Events, Properties, Records, Enums, Actors, Coverage,
Changelog). `cargo xtask tracking` SHALL generate from it the typed events
of each owning crate and the canonical mirror `tracking/tracking-plan.json`
(with a JSON Schema per event), and `--check` SHALL fail when either output
differs from what the workbook produces. `cargo xtask tracking apply
<patch.yaml>` SHALL add or update rows, and `fmt` SHALL rewrite the workbook
in canonical form.

#### Scenario: An event added to the workbook only
- **WHEN** a row is added to Events and the generated files are not regenerated
- **THEN** `cargo xtask tracking --check` fails, naming the stale file

#### Scenario: The mirror edited by hand
- **WHEN** `tracking/tracking-plan.json` is changed without changing the workbook
- **THEN** the check fails

### Requirement: Events carry no personal data
Every event property and snapshot field SHALL have a type from the closed list
(no free text; `code` needs a pattern) and a classification. The check SHALL
reject any field classified `personal` or `content`.

#### Scenario: A subject line in a snapshot
- **WHEN** the Records sheet gives `conversations.conversation` a `subject` field classified `content`
- **THEN** the check fails, naming the record and field

#### Scenario: A free-text code without a pattern
- **WHEN** a property of type `code` has no pattern
- **THEN** the check fails

### Requirement: Tracking happens in the transaction of the change
`Tracker::track(&mut tx, &event)` SHALL write the event to
`kernel.tracked_events` in the caller's transaction, only for types generated
from the plan, and only into the workspace the transaction works in. With
`CAMPBOOKS_TRACKING__SINK=none` it SHALL write nothing.

#### Scenario: The change rolls back
- **WHEN** a handler tracks an event and its transaction rolls back
- **THEN** no row exists in `kernel.tracked_events`

#### Scenario: Another workspace's event
- **WHEN** a transaction working in workspace A tracks an event of workspace B
- **THEN** `track` fails and nothing is written

#### Scenario: Sink off
- **WHEN** the sink is `none`
- **THEN** `track` succeeds and writes no row

### Requirement: The kernel adds the envelope
Every tracked event SHALL carry an id (UUIDv7), name, version, occurrence
time, workspace, actor (`user`, `client`, `assistant`, `automation`,
`system`, `admin` or `anonymous`, with an id where the type has one),
optional `on_behalf_of`, the `operationId` when inside an operation, a
correlation id, a causation id when caused by a job or event, the server
role, the app version and the plan version, all taken from the ambient
context and never from the caller.

#### Scenario: A member archives a conversation
- **WHEN** Sam archives a conversation through the API
- **THEN** one `conversations.conversation_archived` event is written with actor `user` and Sam's account id, operation `conversations.archive`, the request's trace id as correlation, and a snapshot of the conversation as it is after archiving

#### Scenario: Work caused by a request
- **WHEN** a job enqueued while serving Sam's request tracks an event
- **THEN** its actor is `system` with the job's kind, `on_behalf_of` is Sam, and its correlation equals the request's

#### Scenario: No context
- **WHEN** an event is tracked outside any request or job
- **THEN** it is written with actor `system` and the command's name

### Requirement: Coverage of operations and jobs
The Coverage sheet SHALL map operations, jobs, schedules and event handlers to
the events they emit, or exempt them with a reason. The check SHALL fail on a
row naming an unknown operation, kind or event, on a row with both events and
an exemption or neither, and on a `live` event whose type is never tracked
outside tests. It SHALL report mutating operations and registered kinds with
no row.

#### Scenario: A stale coverage row
- **WHEN** a Coverage row names an `operationId` that no longer exists
- **THEN** the check fails, naming the row

#### Scenario: A live event never tracked
- **WHEN** an event is `live` but its type is not used outside generated code and tests
- **THEN** the check fails

### Requirement: The outbox is published to RabbitMQ
With `CAMPBOOKS_TRACKING__SINK=amqp`, the worker role SHALL publish every
outbox row to the configured topic exchange with routing key
`<name>.v<version>`, the event id as `message_id`, and the envelope and body
as JSON. It SHALL mark a row published only after the broker confirms it.

#### Scenario: An archived conversation reaches the stream
- **WHEN** Sam archives a conversation with the sink `amqp`
- **THEN** one message with routing key `conversations.conversation_archived.v1` and Sam's event id as `message_id` can be read from the stream, and the outbox row is marked published

#### Scenario: The broker is down
- **WHEN** the broker is unreachable while events are tracked
- **THEN** the rows stay unpublished, the relay retries with backoff, and once the broker is back every row is published

#### Scenario: Two workers relay at once
- **WHEN** two worker processes run the relay against the same outbox
- **THEN** each row is published, and no row is held by both at once

### Requirement: The relay declares its topology when asked
With `CAMPBOOKS_TRACKING__DECLARE_TOPOLOGY` on, the relay SHALL declare the
exchange, a stream queue kept for `STREAM_MAX_AGE`, and their binding,
idempotently. With it off, it SHALL declare nothing.

#### Scenario: A restart
- **WHEN** the relay starts against a broker where the topology exists
- **THEN** it starts publishing and changes nothing

### Requirement: Published rows are pruned
`kernel.prune_tracked_events` SHALL delete published rows older than
`CAMPBOOKS_TRACKING__PUBLISHED_RETENTION_HOURS`, and SHALL never delete an
unpublished row. Purging a workspace SHALL delete its outbox rows.

#### Scenario: The broker has been down for days
- **WHEN** rows have been unpublished for longer than the retention
- **THEN** the prune keeps them

### Requirement: The broker password can come from a file
With `CAMPBOOKS_TRACKING__AMQP_PASSWORD_FILE` set, the relay SHALL connect
with the password held in that file (a trailing newline dropped) in place of
any password in `CAMPBOOKS_TRACKING__AMQP_URL`. With the sink `amqp`, a
missing, unreadable or empty file SHALL stop the server at start with a
message that names the setting and never shows the URL or the password.

#### Scenario: A URL without its password
- **WHEN** the URL names the user `campbooks-relay` and no password, and the file holds the user's password
- **THEN** the relay signs in to the broker and publishes

#### Scenario: A wrong password in the file
- **WHEN** the URL carries the right password and the file a wrong one
- **THEN** the broker refuses the relay: the file's password is the one used

#### Scenario: The file is missing
- **WHEN** the sink is `amqp` and the file does not exist
- **THEN** the server does not start, and its message names `CAMPBOOKS_TRACKING__AMQP_PASSWORD_FILE` and not the URL

### Requirement: The conversations module's changes are tracked
Every change the `conversations` module makes on someone's behalf SHALL be
tracked as an event of the tracking plan, in the transaction of the change,
with a snapshot of the records it touched read from the module's own rows
after the change: a conversation's flags, stream and labels; its need's
verdicts; drafts and their sends; taught rules and their runs; rule
suggestions and person notes; applied provider batches; the model pass and
Scout's reads; inboxes created and closed. A change that changes nothing
SHALL track nothing. Events SHALL carry ids, enums, booleans, counts and
times only: never a name, address, subject, body, snippet, label name, note,
rule condition or free-text error. Every operation, job, schedule and event
handler of the module SHALL have a Coverage row.

#### Scenario: A member stars a conversation
- **WHEN** Sam stars a conversation that has no starred message
- **THEN** one `conversations.conversation_starred` event is written with actor `user` (Sam), operation `conversations.star` and the conversation afterwards (starred)

#### Scenario: Starring it again
- **WHEN** Sam stars the same conversation again
- **THEN** nothing more is tracked

#### Scenario: A send fails at the provider
- **WHEN** the connector reports that a released send failed with `needs_reauth`
- **THEN** one `conversations.draft_send_failed` event is written with actor `system`, error `needs_reauth` and the draft's ids, kind, state and recipient counts, and no recipient address or text

#### Scenario: A rule run ends
- **WHEN** applying a taught rule to an inbox's mail finishes
- **THEN** one `conversations.rule_run_finished` event is written with the run's counts, not one per conversation

### Requirement: The identity module's changes are tracked
Every change the `identity` module makes SHALL be tracked as an event of the
tracking plan, in the transaction of the change, with a snapshot of the
records it touched read from the module's own rows after the change: sign-ins
that succeed or are refused (with the kind of refusal), accounts created,
sessions ended one by one or all at once, ways in and passkeys added, renamed
and removed, accounts suspended and unsuspended, apps registered, given a new
secret and removed, consents granted, denied and revoked, held mailbox grants
discarded and adopted, authorization codes exchanged for tokens, refresh
tokens reused and revoked. A change that changes nothing SHALL track nothing.
Events SHALL carry ids, enums, booleans, counts and times only: never an
address, a name, a provider subject, a token, a secret, a code, a credential
id, a user agent, a URL or an IP address. Steps taken before anyone holds a
token SHALL be tracked as actor `anonymous`, with the account by id in the
body. Every mutating operation, handler and schedule of the module SHALL
have a Coverage row.

#### Scenario: A person signs in with a provider
- **WHEN** a known provider identity comes back to the callback and its account may sign in
- **THEN** one `identity.sign_in_succeeded` event is written with actor `anonymous`, operation `identity.finish_sign_in` and the new session (its id, the account id, method `provider` and the provider), and no address

#### Scenario: A sign-in is refused
- **WHEN** the person cancels at the provider
- **THEN** one `identity.sign_in_failed` event is written with failure `cancelled_at_provider`, the provider, and no account and no address

#### Scenario: Signing out everywhere
- **WHEN** a person signs out everywhere with two live sessions
- **THEN** one `identity.sessions_ended` event is written with actor `user` and `sessions_ended` 2

#### Scenario: An app whose id is a URL
- **WHEN** an app known by its metadata document (its client id is a URL) is let in and exchanges its code, or a request runs as a `client` whose id Campbooks did not mint
- **THEN** its events name it by kind (`metadata_document`) and, as an actor, as `{"type": "client"}` with no id; the URL reaches no tracked event

#### Scenario: Refreshing tokens
- **WHEN** a client rotates its refresh token
- **THEN** nothing is tracked; presenting the rotated token again tracks `identity.refresh_reuse_detected`

### Requirement: The web BFF's changes are tracked
Every web session the BFF starts or ends SHALL be tracked as an event of the
tracking plan, in the transaction that writes or deletes the session row:
a session started by a sign-in, a session ended (by logging out, by signing
in again, or because the BFF found its cookie copied, its scopes outdated,
its refresh refused, its tokens unreadable or its access token expired with
no refresh token), and the sessions of an account or of an identity
provider's session ended at once (back-channel logout, signing out
everywhere, a suspension), with how many ended. Ending a session that is
already gone SHALL track nothing. Events SHALL carry the session id, the
account id (when the session's subject is a Campbooks account id) and the
cause only: never a name, an address, a token, an identity provider's
session id, a user agent, a URL or an IP address. A request forwarded
through `/bff/api/*` or `/bff/admin/*` SHALL be tracked by the operation it
reaches, under that operation's id and as the person or administrator whose
session it is. Every mutating operation, handler and schedule of the BFF
SHALL have a Coverage row.

#### Scenario: Signing out of the web app
- **WHEN** a signed-in person posts `/bff/logout`
- **THEN** one `bff.web_session_ended` event is written with actor `anonymous`, operation `bff.logout`, the session id, the account id and cause `logout`, and logging out again tracks nothing

#### Scenario: Signing in on the web
- **WHEN** the callback finishes a sign-in in a browser that already had a session
- **THEN** a `bff.web_session_ended` event with cause `replaced` is written for the old session and a `bff.web_session_started` event for the new one, both in operation `bff.callback`

#### Scenario: A copied cookie
- **WHEN** a session token rotated away long ago is presented again
- **THEN** one `bff.web_session_ended` event is written with cause `copied_cookie`

#### Scenario: A call through the proxy
- **WHEN** a signed-in person posts to an API operation through `/bff/api/*`
- **THEN** the events that operation tracks carry its own operation id and actor `user` with the person's account id, not `bff.api_post`

#### Scenario: A suspension through the admin proxy
- **WHEN** an administrator suspends an account through `/bff/admin/*` and the account had a web session
- **THEN** `identity.account_suspended` is tracked in `identity.admin_suspend_account` as actor `admin`, and `bff.web_sessions_ended` with cause `account_suspended` and `sessions_ended` 1 as `system` on behalf of that administrator, with the same correlation

### Requirement: The workspaces module's changes are tracked
Every change the `workspaces` module makes SHALL be tracked as an event of
the tracking plan, in the transaction of the change, with a snapshot of the
records it touched read from the module's own rows: workspaces created,
updated, deleted and purged; members joining, changing role, leaving or
being removed, and paused or resumed when their account is suspended or the
suspension lifted; invitations created; signup requests filed, approved and
rejected; signup codes minted, revoked and redeemed. A change that changes
nothing SHALL track nothing. Events SHALL carry ids, enums, booleans, counts
and times only: never a workspace's name, an address, a person's name, a
signup code or its label, a reason, or an invitation token or link; an
invitation SHALL be named by its id and its invitee not at all. Every
mutating operation, handler and job of the module SHALL have a Coverage
row.

#### Scenario: An owner renames a workspace
- **WHEN** an owner changes a workspace's name
- **THEN** one `workspaces.workspace_updated` event is written with actor `user`, operation `workspaces.update`, `fields` `["name"]` and the workspace's snapshot, which holds no name
- **AND** sending the same name again tracks nothing

#### Scenario: Someone is invited and joins
- **WHEN** an admin invites an address and its owner accepts
- **THEN** `workspaces.invitation_created` is written with the invitation's id, role and state `pending`, and `workspaces.member_joined` with `via` `invitation` and the invitation `accepted`; neither holds the address

#### Scenario: A member is removed
- **WHEN** an owner removes a member keeping their mail
- **THEN** one `workspaces.member_left` event is written with `how` `removed`, `mail` `keep` and the membership as it stood

#### Scenario: An account is suspended
- **WHEN** identity suspends an account whose user belongs to two workspaces
- **THEN** one `workspaces.member_suspended` event is written in each, with actor `system`; the same change delivered again tracks nothing

#### Scenario: A deleted workspace is purged
- **WHEN** a deleted workspace's grace period ends and the purge job runs
- **THEN** one `workspaces.workspace_purged` event is written in the purge's transaction

### Requirement: The connectors module's changes are tracked
Every change the connectors module makes on someone's behalf SHALL be
tracked as an event of the tracking plan, in the transaction that makes
it, with a snapshot read from the module's own rows in that transaction: a
connection added, reconnected or removed (with why, and what becomes of
its mail), a connection's health changing (with the health before), a
member asking for a connection to sync now, a sync run that stored
changes or moved its stream's phase (one summary per run, with counts,
never one event per message), the outbound changes a run made at the
provider (one summary per run, with counts by how they ended), a calendar
change ending, and a send succeeding or failing (with the kind of
failure). A change that changes nothing SHALL track nothing. Events SHALL
carry ids, enums, booleans, counts and times only: never an address, a
display name, a host, a username, a folder or label name, a subject, a
provider's id or a provider's error text. Every mutating operation, job,
schedule and handler of the module SHALL have a Coverage row.

#### Scenario: Disconnecting a mailbox
- **WHEN** the member who connected a mailbox disconnects it
- **THEN** one `connectors.connection_removed` event is written with actor `user`, operation `connectors.disconnect`, the connection as it stood, cause `disconnected` and the mail choice, and no address

#### Scenario: A sync run
- **WHEN** a stream's run stores a batch of new mail
- **THEN** one `connectors.stream_synced` event is written as `system` with the stream afterwards and the number of changes and batches, and a later run that finds nothing tracks nothing

#### Scenario: A grant stops working
- **WHEN** a run finds the grant revoked at the provider
- **THEN** one `connectors.connection_health_changed` event is written with `before` `ok` and the connection's health `needs_reauth`, reason `grant_revoked`

#### Scenario: A refused send
- **WHEN** the provider refuses a message for good
- **THEN** one `connectors.send_failed` event is written with the send, state `failed`, and error `rejected`

### Requirement: The assistant, attention, search and system modules' changes are tracked
Every change the `assistant`, `attention` and `system` modules make, and the
kernel's job queue changes an administrator makes through `system`, SHALL be
tracked as an event of the tracking plan, in the transaction of the change,
with a snapshot read from the owning module's own rows: chats with Scout
started, continued (a question stored), answered and deleted; Scout's
proposals confirmed, declined and undone; a member's profile updated; an
item put off until later and brought back; a workspace's first refused AI
call of the day; a dead job retried or discarded, the dead jobs of a kind
retried, and a schedule's stuck run discarded. A change that changes
nothing SHALL track nothing. Events SHALL carry ids, record references,
enums, booleans, counts, dates and times only: never a question, an answer,
a prompt, a title or anything a model wrote, a search query, a screen
route, a job's payload or error. A search query, a read and the search
index's upkeep SHALL track nothing. Every mutating operation, job, schedule
and handler of the four modules SHALL have a Coverage row.

#### Scenario: A member asks Scout in a chat
- **WHEN** a member starts a chat about a conversation and asks a question
- **THEN** `assistant.chat_started`, `assistant.chat_continued` and `assistant.answer_finished` are written with actor `user`, the chat's id, what it is about as a record reference, its question count, and the answer's finish and tools; none holds the question or the answer
- **AND** starting the chat about the same conversation again tracks nothing

#### Scenario: A member declines a proposal
- **WHEN** a member says Not now to one of Scout's cards
- **THEN** one `assistant.proposal_declined` event is written with the proposal's kind, action and state `declined`
- **AND** settling it again is refused and tracks nothing

#### Scenario: A member puts an item off
- **WHEN** a member snoozes an item until Monday, then until Tuesday
- **THEN** two `attention.item_snoozed` events are written, the second with `before` Monday
- **AND** snoozing it until Tuesday again tracks nothing

#### Scenario: An administrator retries a dead job
- **WHEN** an administrator retries a dead job
- **THEN** one `kernel.job_retried` event is written with actor `admin`, operation `system.admin_retry_job`, and the job's kind, queue and state `pending`
- **AND** retrying it again tracks nothing

#### Scenario: A workspace reaches its daily AI limit
- **WHEN** a workspace's background AI calls reach the cap less the reserve
- **THEN** one `system.ai_limit_reached` event is written in that workspace with the day, the cap, the reserve and `interactive` false
- **AND** later refusals the same day track nothing

#### Scenario: A member searches
- **WHEN** a member searches their mail
- **THEN** nothing is tracked

### Requirement: The admin module's changes are tracked
Every change to an instance administrator's profile SHALL be tracked as an
instance-wide event of the tracking plan, in the transaction of the change,
with a snapshot of the profile read from the `admin` module's own rows: a
profile granted or given more permissions (`admin.profile_granted`), and
permissions taken away or the whole profile removed
(`admin.profile_revoked`), each with the permissions held before. The
actor SHALL be `system` with the command's name (`admin`). A grant or a
revoke that changes no permission SHALL track nothing. Events SHALL carry
ids, enums, booleans and times only: never an address, a person's name or
the operator's note, and an account id only when it is a Campbooks account
id. Listing the administrators, reading the audit trail and appending to it
SHALL track nothing.

#### Scenario: An operator makes someone an administrator
- **WHEN** an operator grants `system.read` to an account with no profile
- **THEN** one `admin.profile_granted` event is written with no workspace, actor `system` `admin`, `new_profile` true, `before` empty, `granted` `["system_read"]` and the profile's snapshot, which holds the account id and no note
- **AND** granting `system.read` again tracks nothing

#### Scenario: An operator takes a permission away
- **WHEN** an operator revokes `jobs.retry` from a profile holding `jobs.read` and `jobs.retry`
- **THEN** one `admin.profile_revoked` event is written with `before` `["jobs_read", "jobs_retry"]`, `revoked` `["jobs_retry"]` and the profile holding `jobs_read`
- **AND** revoking `jobs.retry` again tracks nothing

#### Scenario: An operator removes an administrator
- **WHEN** an operator revokes every permission of a profile
- **THEN** one `admin.profile_revoked` event is written whose profile has no permissions and a `removed_at`
- **AND** revoking from the account again, now with no profile, tracks nothing

#### Scenario: A profile of another identity provider's account
- **WHEN** an operator grants a permission to an account id that is not a Campbooks account id
- **THEN** the event is written and its snapshot has no `account_id`

### Requirement: The schedule module's changes are tracked
Every change the schedule module makes to a member's calendars, events or
settings SHALL be tracked as an event of the tracking plan, in the
transaction that makes it, with a snapshot read from the module's own rows
in that transaction: an event created (by the member, or by Scout from an
email), edited (which fields, by name), moved (with its times before),
cancelled by a later email, deleted, restored, answered (with the answer
before), added to a provider calendar and taken away again; a calendar
added by a grant, turned on or off, recoloured, or removed with its
connection; the member's settings changed (which settings, by name); and
each batch of a provider's changes the module applied (one summary per
batch, with counts, never one event per event). A change that changes
nothing SHALL track nothing. Events SHALL carry ids, enums, booleans,
counts and times only: never a title, a description, a place, notes, an
attendee, a calendar's name or colour, a time zone, a provider's id, an
etag or an iCalendar UID. Every mutating operation, job and handler of the
module SHALL have a Coverage row; where how a change ends at the provider
is already tracked by connectors, the row names that event.

#### Scenario: Moving an event
- **WHEN** a member changes the start of an event on their Campbooks calendar
- **THEN** one `schedule.calendar_event_moved` event is written with actor `user`, operation `schedule.update_event`, the event afterwards and its times before, and no title or place

#### Scenario: Renaming an event
- **WHEN** a member changes an event's title and place
- **THEN** one `schedule.calendar_event_edited` event is written naming the fields `title` and `place`, never their values, and sending the same values again tracks nothing

#### Scenario: Scout adds an event from an email
- **WHEN** a read of an email in a member's own inbox carries a flight
- **THEN** one `schedule.calendar_event_created` event is written as `system` with the event's kind `travel`, origin `scout` and the message's id

#### Scenario: A provider batch
- **WHEN** a batch of a provider calendar's changes creates two events and cancels one
- **THEN** one `schedule.provider_changes_applied` event is written with the calendar and the counts 2 created, 0 changed and 1 cancelled, and the same batch again tracks nothing

### Requirement: The tasks module's changes are tracked
Every change the tasks module makes to a task SHALL be tracked as an event
of the tracking plan, in the transaction that makes it, with a snapshot
read from the module's own row in that transaction: a task created (by a
member, or by Scout from an ask in an email), edited (which fields, by
name), its due date changed (with the date before), planned (with the plan
before), done, not needed, reopened (with the state before), and
reassigned: handed off to a teammate or taken back (with the member who
had it). A change that
changes nothing SHALL track nothing, including a plan set again to what it
was. Events SHALL carry ids, enums, booleans, counts, dates and times only:
never a title, a sender's address, an amount or a currency, a quote from
the email or a person's name; members appear by their user id. Every
mutating operation, job and handler of the module SHALL have a Coverage
row.

#### Scenario: A member writes a task
- **WHEN** a member makes a task with a title and a due day
- **THEN** one `tasks.task_created` event is written with actor `user`, operation `tasks.create`, the task's id, verb, origin `own`, state `open` and due day, and no title

#### Scenario: Scout reads an ask
- **WHEN** a read of an email in a member's own inbox carries a `pay` ask with an amount
- **THEN** one `tasks.task_created` event is written as `system` with origin `ask`, verb `pay`, the conversation it came from by reference and `has_amount` true, and no title, amount or currency

#### Scenario: Renaming a task and moving its due date
- **WHEN** a member changes a task's title and its due day in one update
- **THEN** one `tasks.task_edited` event names the field `title`, never its value, one `tasks.task_due_changed` event carries the due day before, and sending the same values again tracks nothing

#### Scenario: Handing a task off
- **WHEN** a member hands an open task to a teammate
- **THEN** one `tasks.task_reassigned` event is written with reassignment `handed_off`, whose task has the teammate as `assignee` and the member as `handed_by`, with the member as `previous_assignee`
