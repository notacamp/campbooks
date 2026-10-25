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
