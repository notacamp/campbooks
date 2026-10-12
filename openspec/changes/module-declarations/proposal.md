# Proposal

## Why

ADR 0008 requires every module, from the first one, to declare its record
types, actions and events in its `Module` descriptor, so that the HTTP API,
the client's shortcuts and command menu, and later MCP tools, assistant tools
and automation are generated from one place instead of being registered by
hand on each surface. The first real modules (`conversations`, `workspaces`)
are next; they need the declaration types, the action registry and the
generated endpoints to exist first.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/50.

## What Changes

- **Kernel declarations** (`campbooks_kernel::declare`): `RecordRef { type,
  id }`; `RecordType` (name, access container, label key, JSON Schema of its
  public shape); a typed `Action` builder (`<module>.<verb>`, input/output
  types, required scope, `mutates`, optional undo with a window, optional
  default shortcut, label key, handler) yielding an `ActionDecl`; events as
  declared `DomainEvent`s with their version and payload schema; a
  `Declarations` value on `Module`.
- **Generated HTTP surface**: each action is `POST /api/v1/actions/<name>`,
  in the OpenAPI document with its schemas and scope; the scope is enforced
  exactly like `Scoped<S>`; bad input is problem+json 422; mutating actions
  honour `Idempotency-Key`; undoable actions return an undo token redeemed at
  `POST /api/v1/actions/undo`.
- **Kernel tables** `kernel.idempotency_keys` and `kernel.undo_tokens`
  (migration `20261013120000_kernel__actions.sql`), pruned hourly by a new
  kernel schedule, `kernel.prune_actions`.
- **Catalogue**: `campbooks-server declarations` prints every record type,
  action and event as JSON; committed as `openapi/declarations.json` and kept
  current by `cargo xtask declarations --check` in CI.
- **Architecture rules** (`cargo xtask arch`): declarations are named after
  their module and unique, each action requires one of its module's scopes,
  has input and output schemas, a valid shortcut and a test that calls it by
  name; the module name `actions` is reserved.
- **Reference action** `system.whoami` (scope `system:read`, read-only): who
  the API takes the caller to be.
- `kernel::Error` gains `Unprocessable` (422). `Module` gains a
  `declarations` field (every module literal adds it). `testing::app` mounts a
  module's action endpoints; `testing::call_action` calls one by name.

## Capabilities

### New Capabilities

- `declarations`: declaring record types, actions and events; the generated
  action endpoints (scope, validation, idempotency, undo); the catalogue.

### Modified Capabilities

None. Health and version are unchanged.

## Impact

- Crates: `campbooks-kernel` (new `declare` module, `names`, error variant,
  auth helper shared by `Scoped<S>` and actions, registry schedules),
  `campbooks-server` (catalogue, action routes, `declarations` command,
  startup check), `campbooks-system` (`whoami` action), `xtask`
  (`declarations` command, new arch rules). No new module, no cross-module
  dependency, no `architecture.toml` entry.
- Data: two tables in schema `kernel`, owned by the kernel.
- Dependencies: no new crate; utoipa's `debug` feature (Debug for schema
  types).
- OpenAPI: new operation `system_whoami`; client types regenerated.
- Self-hosters: nothing to do; the migration runs on startup.

## Non-goals

- MCP tools, assistant tools and automation. The catalogue carries what they
  need (descriptions, schemas, scopes, mutates, undo, events); they are
  generated in their own changes.
- The client's shortcut registry and command menu (the design-system change
  reads `openapi/declarations.json`).
- Workspace context in actions. `ActionContext` carries the principal; the
  workspace joins it when the `workspaces` module lands (#18), without
  changing handler signatures.
- Real record types, actions and events of domain modules (none exist yet
  beyond `system.whoami`).
- Checking that label keys exist in the clients' i18n files.
- Redo, batch actions, and per-workspace rate limits on actions.
