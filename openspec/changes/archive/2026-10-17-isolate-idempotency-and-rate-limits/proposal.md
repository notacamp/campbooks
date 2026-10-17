# Proposal

## Why

The security review of notacamp/campbooks#526 (campbooks-cloud#207) found
two kernel tables that predate it with no row-level security, tracked as
campbooks-cloud#210:

- **`kernel.idempotency`** stores the response of every caller's idempotent
  request, the one-time `client_secret` in `identity.register_client`'s 201
  included. Any transaction of the runtime role (any workspace, through a
  code mistake) could read another caller's cached secret, or write a forged
  response that a later replay would serve.
- **`kernel.rate_limits`** could be read, reset or exhausted by any
  transaction, which defeats the public endpoints' limits and the per-person
  send limit (#504).

The issue also asks for a check that no other kernel table is open the same
way.

Modules touched: the kernel (`idempotency`, `rate_limit`, `db`, the
operation metadata, a migration), `identity`, `workspaces` and
`connectors` (five operations declare that they show a secret once), the
server (tests) and `xtask` (the arch rules). No new module and no new
cross-module dependency.

## What Changes

- **Idempotency keys belong to their caller.** `kernel.idempotency` gets
  forced row-level security: the idempotency layer reaches a key only in a
  transaction naming its caller (`campbooks.idempotency_caller`, set only by
  the kernel's `begin_as_idempotency_caller`); the kernel scope may read and
  delete expired keys and nothing else (the hourly prune). A workspace
  transaction, or one that set nothing, sees and writes no key.
- **A secret is never cached.** A new operation flag,
  `x-campbooks.secret_once` (`Operation::secret_once()`), marks an
  operation whose successful answer shows a secret once:
  `identity.register_client`, `identity.rotate_client_secret`,
  `workspaces.invite_member` (the accept link), `identity.start_link` and
  `connectors.start_connection` (single-use tickets). For these the layer
  stores only that the key was used (`withheld`: status and `Location`, no
  body; the table refuses a `withheld` row with a body), and a replay of the
  key answers **409 `idempotency_secret_shown_once`** (marked replayed)
  instead of running the operation again or showing the secret twice. The
  migration deletes the cached responses of those five operations.
- **Rate-limit counters are reached through the kernel's functions only.**
  The runtime role loses every privilege on `kernel.rate_limits`; it counts
  through `kernel.take_rate_limit(bucket, window_seconds, cost)` and prunes
  through `kernel.prune_rate_limits(batch)`, `SECURITY DEFINER` functions
  with a pinned `search_path` that validate their input (a 32-byte bucket, a
  window of at most a day, a cost of at least one). The table has forced
  row-level security (kernel scope only) on top.
- **Every kernel table is covered.** A server test asserts that every table
  of the `kernel` schema enables and forces row-level security with a
  policy, or is listed as instance-only with its reason (one today:
  `kernel.instance_rows_from_workspaces`, read-only to the runtime role).
- **Arch rules** (`cargo xtask arch`): `x-campbooks.secret_once` is a
  boolean on mutating operations only; the idempotency caller's setting and
  constant are named only in the kernel, like the scopes'; a migration may
  pin a function's `search_path` (`SET search_path = pg_catalog, pg_temp`),
  and still may not change the session's.
- **Migration** `20261021060000_kernel__idempotency_and_rate_limits_isolation.sql`,
  one transaction with a lock timeout.

## Non-goals

- Moving the idempotency table behind functions or a separate database role
  too. Its isolation rests on a setting, like the workspace's: a defence
  against code mistakes, not against arbitrary SQL (see `design.md`).
- Row-level security on the BFF's tables (`bff.sessions`,
  `bff.login_transactions`, `bff.retired_tokens`): another schema, reached
  only by the BFF's reviewed `store.rs`; not part of this issue.
- A per-workspace column on either table: keys and counters belong to
  callers (people, apps, addresses), not workspaces.

## Impact

- A replay of one of the five operations answers 409 instead of the stored
  answer. A client that lost the first answer starts again: rotate the
  secret, invite again (it replaces the pending invitation), or start a new
  link or connection (their tickets last two minutes anyway).
- Cached responses of those five operations made before the migration are
  dropped; a retry of one of them within a day runs it again.
- Each idempotent request opens two short transactions (claim, then store
  or release) instead of autocommit statements; a rate-limit hit stays one
  statement.
