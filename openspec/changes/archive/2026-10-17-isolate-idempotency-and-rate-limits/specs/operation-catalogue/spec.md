## MODIFIED Requirements

### Requirement: Operations are declared in the OpenAPI document
Every HTTP operation SHALL have an `operationId` of the form
`<module>.<verb>[_<noun>]`, where `<module>` is the module whose router serves
it, unique across the document. Every operation SHALL carry an `x-campbooks`
extension with `mutates` (boolean), `automation` (boolean), `assistant`
(boolean), `risk` (`low` or `high`), `records` (a list of the module's own
`<module>.<record>` types) and optionally `undo` (an `operationId`) and
`rate_limit` (`send`: the operation is counted in the send class of the
per-caller rate limits instead of the write class) and `secret_once`
(`true`: its successful answer shows a secret once, which the idempotency
layer never stores). An
operation with `mutates: true` SHALL use POST, PUT, PATCH or DELETE, and one
with `mutates: false` SHALL use GET or HEAD. An `undo` SHALL name an existing
mutating operation and SHALL only appear on a mutating operation. A
`rate_limit` SHALL only appear on a mutating operation and SHALL only be
`send`. A `secret_once` SHALL be a boolean and SHALL only appear on a
mutating operation. The architecture check SHALL fail on any violation.

#### Scenario: Health and version are catalogued
- **WHEN** the OpenAPI document is generated
- **THEN** `GET /api/v1/system/health` has `operationId` `system.health` and `GET /api/v1/system/version` has `system.version`, each with an `x-campbooks` block of a non-mutating, low-risk operation callable by neither automation nor the assistant

#### Scenario: An operation of another module's prefix
- **WHEN** a handler served by module `mail` declares `operationId` `calendar.list`
- **THEN** `cargo xtask arch` fails naming the operation and the expected prefix `mail.`

#### Scenario: A dangling undo
- **WHEN** an operation's `x-campbooks.undo` names an operation that does not exist or does not mutate
- **THEN** `cargo xtask arch` fails naming both

#### Scenario: A send class on a read
- **WHEN** a non-mutating operation declares `x-campbooks.rate_limit: send`, or any operation declares another value
- **THEN** `cargo xtask arch` fails naming the operation

#### Scenario: A secret flag on a read
- **WHEN** a non-mutating operation declares `x-campbooks.secret_once: true`, or any operation declares it as a string
- **THEN** `cargo xtask arch` fails naming the operation

### Requirement: Idempotent mutating operations
For every operation whose `x-campbooks.mutates` is true, a request with an
`Idempotency-Key` from an authenticated caller SHALL run at most once per key
and caller while the key is retained (24 hours): an identical retry SHALL
receive the stored response with `Idempotency-Replayed: true`; a different
request (operation, method, path, query or body) with the same key SHALL
receive 422; a retry while the first request is running SHALL receive 409.
Server errors and authentication or rate-limit failures SHALL release the
key. GET and HEAD requests SHALL ignore the header, and no operation SHALL be
able to opt out. Expired keys SHALL be deleted by an hourly job. For an
operation declaring `x-campbooks.secret_once` (`identity.register_client`,
`identity.rotate_client_secret`, `workspaces.invite_member`,
`identity.start_link`, `connectors.start_connection`), a successful answer
SHALL NOT be stored: the key SHALL be kept as used (`withheld`: its status
and `Location`, no body), and a replay SHALL answer 409 problem+json
`idempotency_secret_shown_once` with `Idempotency-Replayed: true`, in the
caller's language, after the same authorization as any replay, without
running the operation again or showing the secret.

#### Scenario: Identical retry
- **WHEN** a client sends the same POST twice with `Idempotency-Key: k-1`
- **THEN** both responses have the same status and body, the second carries `Idempotency-Replayed: true`, and the handler ran once

#### Scenario: Key reused for another body
- **WHEN** a client reuses `Idempotency-Key: k` with a different body
- **THEN** the response is a 422 problem+json

#### Scenario: Server error
- **WHEN** the first request with a key fails with 503
- **THEN** a retry with the same key runs the operation again

#### Scenario: A secret shown once
- **WHEN** an admin registers a confidential app with `Idempotency-Key: register-1` and sends the same request again with the same key
- **THEN** the first answer is 201 with the app's secret, the stored key holds no body and no secret, and the second answer is 409 `idempotency_secret_shown_once` marked replayed, without the secret

#### Scenario: The key of a secret reused
- **WHEN** the key of a withheld answer is reused for another request
- **THEN** the answer is 422, as for any reused key
