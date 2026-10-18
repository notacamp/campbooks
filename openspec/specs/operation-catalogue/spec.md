# operation-catalogue Specification

## Purpose
How every operation Campbooks can perform is declared once, in the generated
OpenAPI document, so the API, clients, automation and the assistant read one
catalogue (ADR 0013).

## Requirements

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
invocation is "confirmed" is said by the code making it. A mutating operation
whose answer below 400 has a field that looks like a secret, at any depth
of its schema, or that redirects (a 3xx, whose `Location` may carry a
ticket), SHALL declare `secret_once`, or the field (or `Location`) SHALL be
listed in the architecture check with the reason it is none; a listed field
no longer answered SHALL be reported. A field looks like a secret by its
words, in any case: split at `_`, `-`, `.`, a space, camelCase, the end of
a run of capitals (`IDToken`) and before a trailing number (`token2`). A
guarded word is `secret`, `token`, `ticket`, `password`, `passwd`,
`passphrase`, `credential`, `code`, `apikey` or `otp`, singular or plural.
An API, private, access, signing or secret key anywhere is guarded, and so
is `url`, `uri`, `link` or `key` as the last word. The architecture check SHALL fail on
any violation.

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

#### Scenario: A secret-looking answer
- **WHEN** a mutating operation without `secret_once` answers 201 with a nested `refresh_token`, `clientSecret`, `api_key`, `invite_link` or `code`, or redirects (303)
- **THEN** `cargo xtask arch` fails naming the operation and the field (or `Location`), unless it is listed with its reason, while `tokenizer`, `codec` or `key_id` pass and `IDToken`, `OTPCode` or `public_key` do not

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
able to opt out. Expired keys SHALL be deleted by an hourly job. For an
operation declaring `x-campbooks.secret_once` (`identity.register_client`,
`identity.rotate_client_secret`, `workspaces.invite_member`,
`identity.start_link`, `connectors.start_connection`, `identity.token`), an
answer below 400 SHALL NOT be stored: the key SHALL be kept as used
(`withheld`: its status, and a success's `Location`, never a redirect's, no
body), and a replay SHALL answer 409 problem+json
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

#### Scenario: The token endpoint and a key
- **WHEN** a client calling `/oauth2/token` also sends a valid bearer token and an `Idempotency-Key`, then sends the same request again
- **THEN** the issued tokens are not stored, and the second request answers 409 `idempotency_secret_shown_once`

### Requirement: Record references
The kernel SHALL provide a record reference `{ type, id }` whose `type` is
`<module>.<record>`, so modules can point at each other's records without
foreign keys.

#### Scenario: Serialized reference
- **WHEN** a reference to message `42` of type `mail.message` is serialized
- **THEN** it is `{"type": "mail.message", "id": "42"}`

### Requirement: Reads that keep their inputs out of URLs
An operation that does not mutate but takes personal data as input (addresses,
subjects) SHALL be served by POST with its inputs in the body and declare
`x-campbooks.query_in = "body"` (`Operation::body_query()`), so the data never
appears in a URL, an access log, a proxy or browser history. `cargo xtask
arch` SHALL accept a non-mutating POST only with that marker, and SHALL
refuse the marker on a mutating operation or another method.

#### Scenario: Previewing a rule
- **WHEN** the catalogue lists `conversations.preview_rule`
- **THEN** it is a POST with `mutates: false` and `query_in: "body"`, and the arch check passes

#### Scenario: A read POST without the marker
- **WHEN** an operation is a POST with `mutates: false` and no `query_in`
- **THEN** `cargo xtask arch` reports that it must be GET or HEAD, or marked

### Requirement: A replay is authorized like the request it repeats
Before a stored response is replayed, the caller SHALL still hold the
operation's scopes (else 403), still belong to the workspace of a
`{workspace_id}` route (a client token still bound to it), and still see
every container the first run checked. Otherwise the answer SHALL be the
404 the operation itself would give, and the operation SHALL NOT run
again.

#### Scenario: Taken out of the inbox
- **WHEN** a member repeats, with the same key, a request whose response is stored, after being taken out of the inbox it acted in
- **THEN** the answer is 404 and no stored body is returned

### Requirement: A handler knows who invoked it
The `Workspace<S>` extractor SHALL tell a handler who ran the request
in-process (`invoked_by`: the assistant or automation, through `invoke`) or
that it arrived over HTTP (`None`). The kernel SHALL set it only from a
request extension that `invoke` attaches, private to the kernel: no
header, token claim or body can set or clear it, and no module can make
one. A request the idempotency layer authenticated SHALL keep how it
arrived.

#### Scenario: The assistant calls an operation
- **WHEN** the assistant invokes an operation for Sam in his workspace
- **THEN** its handler sees `invoked_by` as the assistant, with Sam's access

#### Scenario: Sam's own request
- **WHEN** Sam's client calls the same operation over HTTP with his token, whatever headers it sends
- **THEN** its handler sees `invoked_by` as `None`
