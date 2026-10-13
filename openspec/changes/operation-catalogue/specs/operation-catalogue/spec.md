## ADDED Requirements

### Requirement: Operations are declared in the OpenAPI document
Every HTTP operation SHALL have an `operationId` of the form
`<module>.<verb>[_<noun>]`, where `<module>` is the module whose router serves
it, unique across the document. Every operation SHALL carry an `x-campbooks`
extension with `mutates` (boolean), `automation` (boolean), `assistant`
(boolean), `risk` (`low` or `high`), `records` (a list of the module's own
`<module>.<record>` types) and optionally `undo` (an `operationId`). An
operation with `mutates: true` SHALL use POST, PUT, PATCH or DELETE, and one
with `mutates: false` SHALL use GET or HEAD. An `undo` SHALL name an existing
mutating operation and SHALL only appear on a mutating operation. The
architecture check SHALL fail on any violation.

#### Scenario: Health and version are catalogued
- **WHEN** the OpenAPI document is generated
- **THEN** `GET /api/v1/system/health` has `operationId` `system.health` and `GET /api/v1/system/version` has `system.version`, each with an `x-campbooks` block of a non-mutating, low-risk operation callable by neither automation nor the assistant

#### Scenario: An operation of another module's prefix
- **WHEN** a handler served by module `mail` declares `operationId` `calendar.list`
- **THEN** `cargo xtask arch` fails naming the operation and the expected prefix `mail.`

#### Scenario: A dangling undo
- **WHEN** an operation's `x-campbooks.undo` names an operation that does not exist or does not mutate
- **THEN** `cargo xtask arch` fails naming both

### Requirement: Events catalogue
Modules SHALL declare the domain events they publish. The server SHALL
generate a catalogue listing each declared event's name, publishing module,
schema version, whether it is workspace-scoped, and its payload JSON Schema,
committed as `openapi/events.json` and checked in CI. Declared event names
SHALL be `<module>.<event>` of the declaring module, with a version of at
least 1, declared once. Every event a registered handler consumes SHALL be
declared by some module at the version the handler reads.

#### Scenario: A declared event is catalogued
- **WHEN** a module declares an instance-wide event `test.rotated` version 3 whose payload references another schema
- **THEN** the catalogue lists it with module `test`, version 3, `workspaceScoped: false`, its payload schema, and the referenced schema under `components.schemas`

#### Scenario: A consumer of an undeclared event
- **WHEN** a handler consumes an event no module declares
- **THEN** `cargo xtask arch` fails naming the handler and the event

### Requirement: Invoke an operation by id
The kernel SHALL run an operation, given its `operationId`, an input, a
principal, a caller kind (automation or assistant), an optional workspace and
an optional idempotency key, through the same router HTTP requests reach.
Before dispatching it SHALL refuse unknown operations, operations whose
`x-campbooks` does not allow the caller kind, high-risk operations not
confirmed by the user, and mutating operations without an idempotency key.
The operation's scope, membership and record checks SHALL apply exactly as
for an HTTP request carrying the principal's token. The principal SHALL only
be attachable in-process.

#### Scenario: A caller the operation does not opt in
- **WHEN** automation invokes an operation whose `x-campbooks.automation` is false
- **THEN** the call is refused before the handler runs

#### Scenario: A missing scope
- **WHEN** the assistant invokes an allowed operation for a principal lacking the operation's scope
- **THEN** the call fails with status 403 and the `insufficient_scope` problem, as over HTTP

#### Scenario: A retried mutation
- **WHEN** automation invokes the same mutating operation twice with the same idempotency key and input
- **THEN** the handler runs once and the second call returns the stored result marked as replayed

### Requirement: Idempotent mutating operations
For every operation whose `x-campbooks.mutates` is true, a request with an
`Idempotency-Key` from an authenticated caller SHALL run at most once per key
and caller while the key is retained (24 hours): an identical retry SHALL
receive the stored response with `Idempotency-Replayed: true`; a different
request (operation, method, path, query or body) with the same key SHALL
receive 422; a retry while the first request is running SHALL receive 409.
Server errors and authentication or rate-limit failures SHALL release the
key. GET and HEAD requests SHALL ignore the header, and no operation SHALL be
able to opt out. Expired keys SHALL be deleted by an hourly job.

#### Scenario: Identical retry
- **WHEN** a client sends the same POST twice with `Idempotency-Key: k-1`
- **THEN** both responses have the same status and body, the second carries `Idempotency-Replayed: true`, and the handler ran once

#### Scenario: Key reused for another body
- **WHEN** a client reuses `Idempotency-Key: k` with a different body
- **THEN** the response is a 422 problem+json

#### Scenario: Server error
- **WHEN** the first request with a key fails with 503
- **THEN** a retry with the same key runs the operation again

### Requirement: Record references
The kernel SHALL provide a record reference `{ type, id }` whose `type` is
`<module>.<record>`, so modules can point at each other's records without
foreign keys.

#### Scenario: Serialized reference
- **WHEN** a reference to message `42` of type `mail.message` is serialized
- **THEN** it is `{"type": "mail.message", "id": "42"}`
