# Proposal

## Why

The security review of the instance-rows change (notacamp/campbooks#522,
campbooks-cloud#196) left findings that campbooks-cloud#207 tracks:

- **A workspace can still forge instance-wide work.** The queue's and the
  event log's `instance_rows_insert` policy lets any workspace transaction
  INSERT a row that belongs to no workspace. A raw INSERT of
  `kind = 'kernel.prune_jobs', unique_key = 'schedule:kernel.prune_jobs',
  run_at = 'infinity'` makes every later tick of that schedule a duplicate
  (`ON CONFLICT DO NOTHING`), so pruning stops for good; the same row for
  `connectors.dispatch_due` stops every stream's polling. Nothing deletes it:
  the workspace can no longer change the row, and the admin's discard only
  removes failed jobs. The unique index spans workspaces, so the workspace's
  own rows can do the same. Instance-wide account events can be forged for
  their consumers the same way.
- **The dispatchers leave the last workspace set** after their enqueue loop:
  whatever the transaction runs next runs in the last stream's or inbox's
  workspace.
- **The server's policy test can miss a policy:** it reads `USING` only,
  skips the kernel's schema, and matches the setting with its comparison.
- **The arch rule's text checks** do not decode escapes inside a literal
  (`"campbooks.kernel\x5fscope"`), have no bare-word rule for `admin_reads`,
  and cut a file at a test module nested in another module or a macro body,
  or one followed by more code.
- **The BFF's `pool_access` entry for `mod.rs`** exempts the whole file for
  one transaction and three connections.

Modules touched: the kernel (`db`, `jobs`, `events`, the registry, a
migration), `identity` (two events declare where they are published),
`connectors` and `conversations` (the dispatchers), `system` (one admin
operation), the server (BFF and tests) and `xtask` (the arch rules). No new
module and no new cross-module dependency.

## What Changes

- **Workspaces cannot forge instance-wide rows.** A workspace transaction
  may add an instance-wide row only as a fresh event whose name is listed in
  the new kernel table `kernel.instance_rows_from_workspaces`, or as a fresh
  delivery of such an event to a listed consumer (pending, unclaimed, due
  now, keyed by its event, no ordering key). The list holds what the
  registry derives: events declared with the new
  `DomainEvent::PUBLISHED_IN_WORKSPACES` (today `identity.consent_revoked`
  and `identity.way_in_removed`) and their consumers (none today). The
  runtime role may only read it; it changes by migration, and a server test
  keeps it equal to the registry. No row, a workspace's own included, may
  carry a schedule's unique key (`schedule:%`) outside the kernel scope. The
  kernel refuses an instance-wide job, and an undeclared instance-wide
  event, from a workspace transaction before asking the database.
- **The schedules' last ticks are the kernel scope's** (row-level security
  on `kernel.schedules`; before, any transaction could set one far ahead
  and stop that schedule for good), and the tick takes over a last tick
  more than one period ahead.
- **An event is handled only where it was logged as its own:** a delivery
  whose event row sits in another workspace than the event's own (an
  account's event forged into a workspace) fails without running its
  handler.
- **Stuck schedule rows heal.** Before a tick enqueues, the kernel marks
  failed (kept for inspection, with the reason) any row holding the
  schedule's key that belongs to a workspace, is due later than any tick or
  retry would put it, or is leased longer than any worker leases. An
  administrator with `jobs.retry` can also discard a schedule's pending or
  stuck run (`DELETE /admin/v1/system/schedules/{name}/run`,
  `system.admin_discard_schedule_run`); a run a live worker holds is kept.
- **The dispatchers clear the workspace.** `jobs::enqueue_in` /
  `enqueue_to_in` set the workspace for one enqueue and clear it again, in
  the same transaction; both dispatchers use them.
- **The server's policy test reads every policy**, `USING` and
  `WITH CHECK`, in every schema, the kernel's included, and asserts the
  kernel scope appears only in the kernel's `workspace_isolation` policies
  and the maintenance scope only in the four maintenance policies.
- **Arch rules** (`cargo xtask arch`): escapes in a non-raw literal are
  decoded before matching a scope's setting or word; `admin_reads` gets the
  same bare-word rule; the test-module cut is taken only at the file's top
  level and only when nothing but whitespace or comments follows it.
- **The BFF:** `bff/mod.rs` loses its `pool_access` entry; the refresh's
  transaction and the session deletes on the pool moved into `store.rs`,
  already the BFF's one exempt file.
- **Migration** `20261021040000_kernel__instance_rows_from_workspaces.sql`,
  one transaction with a lock timeout.

## Non-goals

- Making unique and ordering keys per workspace. The unique index
  `(kind, unique_key)` and ordering keys span workspaces, so a workspace
  can still squat another workspace's key if it knows it (they embed ids).
  That is a separate change of the queue's indexes, noted in `design.md`.
- Checking the payload of a listed instance-wide event: a workspace may
  still append `identity.consent_revoked` or `identity.way_in_removed` with
  any payload. Neither has a consumer; a future consumer must not trust one.
- An admin UI for the new operation (the API only; the Jobs panel can add
  it later).

## Impact

- A workspace transaction that tries to add any other instance-wide row,
  or a schedule-keyed row of its own, fails (`42501`); through the kernel's
  API it fails earlier with `InvalidArgument`.
- A schedule blocked by a forged or wedged row runs again at its next tick.
- One extra statement per tick (the heal) and two per dispatched enqueue
  (setting and clearing the workspace).
