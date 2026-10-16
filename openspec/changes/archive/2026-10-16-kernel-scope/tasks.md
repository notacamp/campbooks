# Tasks

## 1. Kernel

- [x] 1.1 `KERNEL_SCOPE_SETTING`, `begin_kernel`, `JobContext::kernel_tx`; crate-private `enter_kernel_scope`, `leave_kernel_scope`, `begin_in`, `scope`, `require_scope`, `lift_for_instance_rows`, `lower`
- [x] 1.2 Worker bookkeeping (claim, reap, tick, heartbeat, release, fail, sample, completion) in the kernel scope; an instance-wide job's completion lifted in its own transaction
- [x] 1.3 Event delivery loads its event in the delivery's scope; replay in the kernel scope; publish lifts an unset transaction for an instance-wide event
- [x] 1.4 Enqueue refuses a workspace's job outside its workspace or the kernel scope; lifts for an instance-wide one
- [x] 1.5 Change feed: `record` and `purge_workspace` refuse; the stream reads in its workspace; `PruneChanges` in `kernel_tx`
- [x] 1.6 Blobs: `touch` and `purge_workspace` in the workspace; the sweep in the kernel scope; attach and detach refuse
- [x] 1.7 Pruning (`PruneJobs`, `PruneEvents`) and the admin queue in the kernel scope; `jobs::purge_workspace` refuses

## 2. Migrations

- [x] 2.1 `20261020200000_kernel__kernel_scope.sql`: the six kernel policies on the kernel scope
- [x] 2.2 `20261020200100_connectors__kernel_scope.sql`: `sync_due` maintenance policies on the kernel scope
- [x] 2.3 `20261020200200_conversations__kernel_scope.sql`: `needs_backfills` maintenance policies on the kernel scope

## 3. Modules

- [x] 3.1 Connectors dispatcher: `begin_kernel`
- [x] 3.2 Needs backfill dispatcher: `kernel_tx`
- [x] 3.3 `architecture.toml`: `[[access.kernel_scope]]` entries for both; `pool_access` for the BFF and the kernel's transaction openers

## 4. Arch rules

- [x] 4.1 `.acquire()` / `Acquire::begin` rejected like `.begin()`
- [x] 4.2 SQL in a same-file `const` resolved
- [x] 4.3 The server and the kernel checked
- [x] 4.4 The kernel scope's name and openers kept to the kernel and the allowlist (no stale entries)

## 5. Tests

- [x] 5.1 Every table with row-level security (from `pg_class`) shows zero rows to a transaction that sets nothing, after seeding a sample of every kind
- [x] 5.2 Refusals: enqueue, record, attach, detach, purges
- [x] 5.3 Maintenance paths under the kernel scope as the runtime role: worker across workspaces and instance-wide, event deliveries, schedules, pruning, sweep, replay, admin queue, both module dispatchers
- [x] 5.4 Arch-rule unit tests for each new check
