## Purpose

How Campbooks modules declare their record types, actions and events (ADR
0008), and what the platform generates from those declarations: one HTTP
endpoint per action (scope, input validation, idempotency, undo) and a
catalogue that clients and tools generate from.

## ADDED Requirements

### Requirement: Declarations are named after their module
Every record type, action and event a module declares SHALL be named
`<module>.<name>` (lower-case snake case) after the declaring module, and
names SHALL be unique per kind across all modules. Every action SHALL have a
description and SHALL require one of the scopes its own module declares. A
record type's access container SHALL be a declared record type. Event
versions SHALL be at least 1. Shortcuts SHALL follow the shortcut grammar and
be unique within their module. No two declared schemas SHALL share a
component name with different shapes. The server SHALL refuse to start, and
`cargo xtask arch` SHALL fail, when any of these rules is broken, naming each
offending declaration.

#### Scenario: Misnamed action
- **WHEN** module `mail` declares an action named `calendar.hold`
- **THEN** the server refuses to start and the error names `calendar.hold` and says it must be `mail.<name>`

#### Scenario: Action requiring a foreign scope
- **WHEN** an action of module `mail` requires scope `calendar:write`
- **THEN** startup fails with an error naming the action and the scope

#### Scenario: Shortcut bound twice in a module
- **WHEN** two actions of one module declare shortcut `e`
- **THEN** startup fails naming the shortcut and both actions

### Requirement: Record references
A record of any module SHALL be addressable as `{"type": "<module>.<record>",
"id": "<uuid>"}`. Deserializing a reference whose type is not
`<module>.<record>` SHALL fail.

#### Scenario: Malformed reference type
- **WHEN** a client sends `{"type": "Conversation", "id": "…"}` where a record reference is expected
- **THEN** the input is rejected as invalid

### Requirement: Generated action endpoints
Every declared action SHALL be served at `POST /api/v1/actions/<name>` and
SHALL appear in the OpenAPI document as its own operation (operation id
`<module>_<verb>`, tagged with its module) with its input as the request body,
`{"result": <output>}` (plus `undo` for undoable actions) as the 200 response,
problem+json error responses, and a security requirement naming exactly its
scope. A call SHALL require a valid bearer token granting the action's scope,
answering 401 or 403 exactly as other scoped endpoints do, before the body is
read.

#### Scenario: Calling an action
- **WHEN** a caller whose token grants `system:read` posts `{}` to `/api/v1/actions/system.whoami`
- **THEN** the response is 200 with `result` holding the token's subject, kind, client id, scopes and workspace

#### Scenario: Missing scope
- **WHEN** a caller whose token lacks the action's scope calls it
- **THEN** the response is 403 problem+json with `WWW-Authenticate: Bearer error="insufficient_scope", scope="<scope>"` and the action does not run

### Requirement: Action input validation
An action's body SHALL be JSON (`Content-Type: application/json` or
`application/*+json`); an empty body SHALL mean `{}`. A body that is not JSON
SHALL be answered 400, a non-JSON content type 415, a body over 1 MiB 413,
and a body that does not match the action's input 422, all as
`application/problem+json`, without running the action.

#### Scenario: Missing field
- **WHEN** a caller posts `{}` to an action whose input requires `prefix`
- **THEN** the response is 422 problem+json whose `detail` names the missing field `prefix`

### Requirement: Idempotent mutating actions
A mutating action called with an `Idempotency-Key` header (1–255 visible
ASCII characters, optionally quoted) SHALL run at most once per caller and
key for 24 hours: a repeat with the same action and input (compared after
decoding) SHALL receive the first response again with
`Idempotency-Replayed: true`; a repeat with another action, input or
workspace SHALL be answered 422 `idempotency_key_reused`. A call that fails
SHALL store nothing, so the key can be retried. Concurrent calls with one key
SHALL run the action once. A malformed key SHALL be answered 400
`invalid_idempotency_key`. Keys SHALL be per caller. Read-only actions SHALL
ignore the header. The change, its events, its undo token and the stored
response SHALL commit in one transaction.

#### Scenario: Retried request
- **WHEN** a caller sends the same mutating call twice with `Idempotency-Key: k-1`
- **THEN** the action runs once and both responses have the same body, the second with `Idempotency-Replayed: true`

#### Scenario: Key reused for another request
- **WHEN** a caller reuses `k-1` with a different input
- **THEN** the response is 422 with `detail` = `idempotency_key_reused` and the action does not run

#### Scenario: Failed call frees its key
- **WHEN** a call with `Idempotency-Key: k-2` fails and is retried with the same key
- **THEN** the retry runs the action, and nothing from the failed call remains

### Requirement: Undo tokens
An undoable action that changed something SHALL return `undo: {token,
expires_at}`; one that changed nothing SHALL return no `undo`. `POST
/api/v1/actions/undo` with `{"token": …}` SHALL run the action's undo once,
only for the caller the token was issued to in the same workspace, before it
expires, and only if the caller's token still grants the action's scope. An
unknown token or another caller's SHALL be answered 404; a used token 409
`undo_already_used`; an expired token 409 `undo_expired`. Tokens SHALL be
stored only as hashes. Expired tokens and idempotency keys SHALL be deleted
hourly.

#### Scenario: Undo once
- **WHEN** a caller archives a widget, then redeems the returned token twice
- **THEN** the first redemption restores the widget and answers 200 `{"action": "test.archive"}`, and the second answers 409 `undo_already_used`

#### Scenario: Someone else's token
- **WHEN** another caller redeems the token
- **THEN** the response is 404 and nothing is undone

### Requirement: Declarations catalogue
`campbooks-server declarations` SHALL print, as JSON, every declared record
type (name, module, label key, access container, schema), action (name,
module, description, label key, scope, mutates, undoable, undo window,
shortcut, method, path, operation id, input and output schemas) and event
(name, module, version, payload schema), sorted by name, with every
referenced schema under `components.schemas`. The catalogue SHALL be
committed as `openapi/declarations.json`, and CI SHALL fail when it is stale.

#### Scenario: Stale catalogue
- **WHEN** an action is added and `openapi/declarations.json` is not regenerated
- **THEN** `cargo xtask declarations --check` fails, naming the first differing line

### Requirement: Every action is tested
`cargo xtask arch` SHALL fail when a declared action's name, as a string
literal, does not appear in its module's test code, or when an action lacks
a scope from the catalogue or input and output schemas. No module SHALL be
named `actions`.

#### Scenario: Untested action
- **WHEN** no test of module `system` mentions `"system.whoami"`
- **THEN** `cargo xtask arch` fails, saying `system.whoami` has no test
