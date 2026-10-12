# Design

## Context

`Module` is `{ name, scopes, router, background }`. Scopes are `ScopeName`
types enforced by the `Scoped<S>` extractor (#433); domain events are
`DomainEvent`s published in the caller's transaction (#434). ADR 0008 asks
each module to declare record types, actions and events so every surface is
generated from them.

## Data ownership and events

- **Schema `kernel`** (the kernel's, not a module's), migration
  `20261013120000_kernel__actions.sql`:
  - `kernel.idempotency_keys (principal, key)` PK, `action`, `workspace_id`,
    `request_hash bytea` (SHA-256 of the canonical input), `response jsonb`,
    `created_at`, `expires_at` (24 h).
  - `kernel.undo_tokens (token_hash bytea)` PK, `action`, `principal`,
    `workspace_id`, `payload jsonb`, `created_at`, `expires_at`,
    `redeemed_at`. The token itself is never stored.
- **Events**: none published or consumed by this change. Actions publish
  their own events on the action's transaction.
- **Jobs**: `kernel.prune_actions`, scheduled hourly next to
  `kernel.prune_jobs`, deletes expired keys and tokens in batches.

## Endpoints and scopes

| Endpoint | Scope |
| --- | --- |
| `POST /api/v1/actions/<module>.<verb>` | the action's declared scope |
| `POST /api/v1/actions/system.whoami` | `system:read` |
| `POST /api/v1/actions/undo` (exists once an action is undoable) | the scope of the token's action; the OpenAPI operation lists every undoable action's scope as alternatives |

### Why one `/api/v1/actions/<name>` namespace

The alternative was a per-module route (`/api/v1/<module>/actions/<verb>`).
One namespace keyed by the action's name wins because the name *is* the
address: a client that read the catalogue (command menu, assistant,
automation) calls any action with one generic function, with no per-module
URL scheme to learn. Each action is still its own OpenAPI operation (concrete
path, own schemas, own `security`), so generated clients stay typed. Names
always contain a dot, so they never collide with `/api/v1/actions/undo`, and
module routes live under `/api/v1/<module>/`; `cargo xtask arch` reserves the
module name `actions`.

## Decisions

- **Typed builder, type-state.** `Action::<I, O>::new(name)` → `.scope::<S>()`
  → optional `.describe`, `.mutates`, `.undoable(f)`, `.undo_window`,
  `.shortcut`, `.label` → `.handler(h)`. `handler` only exists once a
  `ScopeName` is chosen, so "every action has a scope" holds at compile time.
  `.undoable(f)` changes the handler's return type to `Undoable<O, U>`: the
  handler, which knows the prior state, hands the undo payload `U` to the
  kernel (or `nothing_to_undo`, then no token is issued). Undoable implies
  mutating.
- **Handler signature** `async fn(ActionContext, I) -> Result<O>`. The
  context is passed by value and is extensible (private fields, accessors):
  the workspace becomes one more accessor without touching handlers.
- **The call's transaction.** `ctx.tx()` opens it lazily; the kernel opens
  it up front when the call carries an `Idempotency-Key`. After the handler
  succeeds the kernel writes the undo token and the idempotency response on
  the same transaction and commits; on failure everything rolls back, so a
  failed call stores nothing and its key can be retried. The guard is a
  newtype over tokio's `MutexGuard` (not `MappedMutexGuard`, whose `T: 'a`
  bound defeats `Send` inference in handler futures).
- **Idempotency.** Keys are scoped to the caller (`user:<sub>` /
  `client:<id>`). The claim is an `INSERT … ON CONFLICT DO NOTHING` in the
  call's transaction: a concurrent duplicate blocks on the uncommitted row,
  then replays the stored response (with `Idempotency-Replayed: true`) if the
  first committed, or takes over if it rolled back. A reused key with another
  action, input or workspace is 422 `idempotency_key_reused`. The input is
  canonicalized by decoding it as `I` and re-encoding it before hashing.
  Read-only actions ignore the header.
- **Undo.** Tokens are 122-bit random (`gen_random_uuid()`), stored hashed,
  bound to the caller and workspace, single use (row lock + `redeemed_at`),
  expiring after the action's window (default 10 minutes). Unknown or
  someone else's → 404; used → 409 `undo_already_used`; expired → 409
  `undo_expired`; the caller's token must still grant the action's scope.
- **Validation.** Empty body = `{}`; non-JSON `Content-Type` → 415; over 1 MiB
  → 413; malformed JSON → 400; input not matching `I` → 422 (new
  `Error::Unprocessable`). Auth is checked before the body is read.
- **Catalogue.** `Catalogue::build(&[Module])` validates (owned, unique
  names; description; scope listed in the module's `scopes`; shortcut grammar
  and uniqueness within the module; undo window ≥ 1 s; event version ≥ 1;
  access containers are declared record types; no two schemas share a
  component name) and serializes records, actions and events sorted, with
  `components.schemas` so `$ref`s resolve in the document. The server fails
  to start on an invalid catalogue.
- **Label keys** default to i18next `namespace:key` in the client module's
  namespace: `<module>:actions.<verb>`, `<module>:records.<record>`.
- **Shortcut grammar**: 1–3 chords separated by single spaces, each
  `[mod+][ctrl+][alt+][shift+]<key>` in that order; `mod` = ⌘ on Apple, Ctrl
  elsewhere; keys are a lower-case letter, digit, punctuation, `f1`–`f12` or
  a named key.
- **"Every action has a test"** is checked by convention: the action's name
  as a string literal appears in its module's test code (files in `tests/`,
  `tests.rs`, or after `#[cfg(test)]`), as `testing::call_action` takes it.

## What the catalogue gives later surfaces

| Surface | Uses |
| --- | --- |
| Client shortcuts and command menu | `label`, `shortcut`, `scope` (hide what the token cannot do), `mutates`/`undoable` (confirm or offer undo) |
| MCP tools | `name`, `description`, `input`/`output` schemas, `scope`, `mutates` (read-only/destructive hints) |
| Assistant tools | the same, plus `undoable` to make changes proposals the user can undo |
| Automation | `events` (triggers, with payload schemas and versions), `actions` (steps), `records` (`RecordRef` targets) |

## Risks

- A handler that spawns work holding its `ActionContext` past its return
  would race the kernel's commit; documented (handlers enqueue jobs instead).
- Idempotent responses are stored for 24 h and may contain the action's
  output; outputs hold ids and small values, like job payloads.
