## MODIFIED Requirements

### Requirement: Coverage of operations and jobs
The Coverage sheet SHALL map every mutating operation (in `openapi.json`
and `admin.json`), job, schedule and event handler the server composes,
the kernel's own maintenance included, to the events it emits, or exempt
it with a reason. Coverage SHALL be required in CI with no baseline: the
plan check SHALL fail on a mutating operation with no row, and the server's
coverage test SHALL fail on an operation, job, schedule or handler with no
row, naming each. The check SHALL also fail on a row naming an unknown
operation, kind or event, on a row with both events and an exemption or
neither, and on a `live` event whose type is never tracked outside tests.
A PR that changes the workbook or its JSON mirror SHALL carry the change as
a patch under `tracking/patches/`.

#### Scenario: A stale coverage row
- **WHEN** a Coverage row names an `operationId` that no longer exists
- **THEN** the check fails, naming the row

#### Scenario: A live event never tracked
- **WHEN** an event is `live` but its type is not used outside generated code and tests
- **THEN** the check fails

#### Scenario: A new operation without a row
- **WHEN** a PR adds a mutating operation and no Coverage row for it
- **THEN** the plan check and the server's coverage test fail, naming the operation

#### Scenario: A new job without a row
- **WHEN** a PR registers a job, schedule or event handler and no Coverage row for it
- **THEN** the server's coverage test fails, naming the kind

#### Scenario: The plan changed by hand
- **WHEN** a PR changes `tracking/tracking-plan.xlsx` or `tracking/tracking-plan.json` and no file under `tracking/patches/`
- **THEN** the Policy check fails, pointing to `docs/tracking.md`

## ADDED Requirements

### Requirement: An IMAP mailbox's address checks are tracked
A member asking for an IMAP mailbox's address to be checked again, and the
check's message being sent or failing to send, SHALL be tracked as events
of the tracking plan, in the transaction of the change, with the
connection's snapshot read from the module's rows in that transaction.
Asking while a check is under way, or being refused, SHALL track nothing.
A failure SHALL carry the kind of failure only. Events SHALL never carry
the address, the code, a host, a username or the server's reply.

#### Scenario: Asking again
- **WHEN** a member asks for their mailbox's address to be checked again and no check is under way
- **THEN** one `connectors.address_check_requested` event is written with actor `user`, operation `connectors.check_address` and the connection, and asking again while that check is under way tracks nothing

#### Scenario: The check's message goes out
- **WHEN** the mailbox's SMTP server takes the check's message
- **THEN** one `connectors.address_check_send_completed` event is written as `system` with the connection and outcome `sent`

#### Scenario: The server will not send as the address
- **WHEN** the mailbox's SMTP server refuses to send from or to the address
- **THEN** one `connectors.address_check_send_completed` event is written with outcome `failed` and reason `address_refused`, and no address
