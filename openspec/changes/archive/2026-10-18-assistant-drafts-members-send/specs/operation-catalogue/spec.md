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
mutating operation. An operation with `risk: high` SHALL NOT opt
automation or the assistant in (`automation` and `assistant` false): a
high-risk operation is the person's own request only, since whether an
invocation is "confirmed" is said by the code making it. The architecture
check SHALL fail on any violation.

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

#### Scenario: A high-risk operation opted in for an invoker
- **WHEN** an operation declares `x-campbooks.risk: high` with `assistant: true` or `automation: true`
- **THEN** `cargo xtask arch` fails naming the operation and the caller kind
