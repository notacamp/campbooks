# Proposal

## Why

Row-level security is the second layer under the access model (ADR 0011,
ADR 0017). Module tables were already strict: a transaction that sets no
workspace sees none of their rows. The kernel's tables were not. The event
log, the job queue, the change feed and the blob catalogue admitted every
row to a transaction with no workspace set, because that was how the
kernel's own machinery (workers, pruning, the sweeper, replay) worked across
workspaces. It also meant that any code which forgot `begin_for` (a plain
`pool.begin()`, `acquire()`, `Acquire::begin`) read and wrote every
tenant's events, jobs, changes and blob references with nothing to stop it.
Two module maintenance policies (`connectors.sync_due`,
`conversations.needs_backfills`) keyed on "unset" in the same way.

This is the follow-up the security review of campbooks-cloud#117 asked for
(campbooks-cloud#129).

Modules touched: the kernel (`db`, `jobs`, `events`, `changes`, `blobs`),
`connectors` (the dispatcher), `conversations` (the needs backfill's
dispatcher), the server (tests only) and `xtask` (the arch rules). No new
cross-module dependency.

## What Changes

- **An unset transaction sees nothing.** The kernel tables' policies admit
  cross-workspace access only under an explicit **kernel scope**, the
  transaction-local setting `campbooks.kernel_scope = 'on'`. A transaction
  with neither a workspace nor the kernel scope sees no row of any table
  with row-level security. A workspace transaction still sees its own rows,
  and the instance-wide rows (no workspace) of the event log and the queue.
- **One way in.** `kernel::db::begin_kernel(pool)` and a job's
  `ctx.kernel_tx()` are the only ways to open the scope, and only the
  kernel names the setting. Outside the kernel, a file may call either only
  when `architecture.toml` lists it under `[[access.kernel_scope]]` with the
  reason (reviewed like an ADR): today the connectors' dispatcher and the
  needs backfill's dispatcher.
- **Every legitimate cross-workspace path moves to the kernel scope**, and
  every per-workspace kernel path that ran on the pool (the change feed's
  reader, a blob's catalogue row) moves into its workspace's transaction.
  The list, path by path, is in `design.md`.
- **Instance-wide rows still work from anywhere.** Publishing an account's
  event or enqueueing an instance-wide job from a transaction that set
  nothing (identity's sign-in, a schedule) works: the kernel lifts that
  transaction into its scope for its own insert only.
- **Refusals.** Like `EventBus::publish`, enqueueing a workspace's job,
  recording a change of the feed, attaching or detaching a blob, and the
  kernel's per-workspace purges refuse (`InvalidArgument`) to run in a
  transaction that cannot reach that workspace: one that set none and is
  not in the kernel scope, or one working in another workspace.
- **Maintenance policies.** `connectors.sync_due` and
  `conversations.needs_backfills` keep their read/lease and read/start
  policies, now keyed on the kernel scope instead of "unset".
- **Arch-rule gaps closed.** `cargo xtask arch` now also rejects
  `.acquire()` and `Acquire::begin` on the pool, resolves SQL kept in a
  `const` of the same file, and checks the server and the kernel as well as
  the modules (with reviewed `pool_access` entries for the BFF's
  instance-wide sessions and the kernel's own transaction openers).
- **Migrations** `20261020200000_kernel__kernel_scope.sql`,
  `20261020200100_connectors__kernel_scope.sql` and
  `20261020200200_conversations__kernel_scope.sql` re-create the policies,
  each in one transaction with a lock timeout.

## Impact

- A path that forgot its scope now sees and changes nothing instead of
  every tenant's rows. The kernel paths and the two module dispatchers were
  all moved; a test opens a transaction with no scope on every table with
  row-level security (found from `pg_class`, so new tables are covered) and
  sees zero rows, and each maintenance path is tested under the kernel
  scope as the runtime role.
- One extra round trip (`BEGIN` plus the setting) for the worker's
  bookkeeping statements, which ran straight on the pool before.
