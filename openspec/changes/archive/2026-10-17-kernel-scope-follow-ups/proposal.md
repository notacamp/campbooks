# Proposal

## Why

The kernel scope (campbooks-cloud#129, PR #512) left three things its
review asked to follow up (campbooks-cloud#196):

- The event log's and the queue's `workspace_isolation` policy is
  `FOR ALL`, so a workspace transaction reaching the instance-wide rows (no
  workspace: an account's events and jobs, a schedule's ticks) may also
  UPDATE and DELETE them: complete or delete another job, push a tick into
  next year, rewrite an account's event. Nothing does today, but nothing
  stops it either.
- `cargo xtask arch`'s text checks can be walked around on purpose: a C raw
  string (`cr#"…"#`) is not read as a string, so a `//` inside it hides the
  rest of the line; a setting name built at runtime
  (`'campbooks.' || 'kernel_scope'`) is not caught; the text
  `#[cfg(test)]\nmod tests {` inside a string or a comment ends the check
  early; and the BFF's `pool_access` entry exempts its whole directory, new
  files included.
- The two module dispatchers (connectors' `dispatch_due`, conversations'
  `dispatch_needs_backfills`) get the whole kernel scope, which also
  reaches every workspace's events, jobs, changes and blobs, although they
  need only their own index.

Modules touched: the kernel (`db`, `jobs`), `connectors` (the dispatcher),
`conversations` (the needs backfill's dispatcher), the server (tests only)
and `xtask` (the arch rules). No new module and no new cross-module
dependency.

## What Changes

- **Instance-wide rows change only in the kernel scope.** On `kernel.events`
  and `kernel.jobs`, a workspace transaction still reads the instance-wide
  rows and still inserts them (a workspace handler's instance-wide event or
  follow-up job, a unique-key enqueue, `RETURNING`), but no longer updates,
  deletes or locks them, nor turns its own row into one. The kernel scope
  (the worker, pruning, the admin's queue) still changes them. Every path
  that changes an instance-wide row was checked; the list is in
  `design.md`.
- **A maintenance scope for the module dispatchers.** A new
  transaction-local setting, `campbooks.maintenance_scope = 'on'`, opened
  only by `kernel::db::begin_maintenance` and a job's `maintenance_tx`, is
  trusted by the two maintenance policy pairs (`connectors.sync_due`,
  `conversations.needs_backfills`) and by no kernel table. The dispatchers
  move to it and enqueue each run in its workspace. No module table admits
  the kernel scope any more, and `[[access.kernel_scope]]` is empty; the two
  dispatchers are listed under a new `[[access.maintenance_scope]]`.
- **Arch-rule gaps closed** (`cargo xtask arch`): C raw strings are strings;
  a scope setting's last word on its own (`kernel_scope`,
  `maintenance_scope`, in code or a string, any case, tests included) is
  rejected outside the kernel and the reviewed files; the test-module cut is
  looked for in code only; the BFF's `pool_access` names its two files, so a
  new BFF file is checked.
- **Migrations** `20261021010000_kernel__instance_rows_read_insert_only.sql`,
  `20261021010100_connectors__maintenance_scope.sql` and
  `20261021010200_conversations__maintenance_scope.sql`, each in one
  transaction with a lock timeout.

## Non-goals

- Narrowing what a workspace transaction may read of the instance-wide rows.
  The unique-key `ON CONFLICT`, instance-wide follow-ups and `RETURNING`
  need it.
- A type-aware arch check. What a text check still cannot catch is listed
  in `design.md`; row-level security stays the backstop.
- Any change to who may call `begin_kernel` inside the kernel.

## Impact

- A workspace transaction that tries to change an instance-wide job or event
  now changes nothing (row-level security filters the row out), and one that
  tries to turn its own row into an instance-wide one fails.
- A bug in a module dispatcher can now reach only its own index (ids,
  times, states) and, after setting a workspace, that workspace's rows; it
  no longer reaches any kernel table across workspaces.
- One extra statement per dispatched run (setting its workspace).
