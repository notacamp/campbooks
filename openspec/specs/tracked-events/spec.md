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
