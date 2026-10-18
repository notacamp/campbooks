## 1. Tombstones and the batched purge

- [x] 1.1 Migration: `kernel.purged_workspaces` with the standard `workspace_isolation` policy and the runtime role's grant
- [x] 1.2 `kernel::purge::purge_workspace`: 5,000 rows per table per short transaction in the workspace; `jobs::purge_workspace` and `changes::purge_workspace` take a limit
- [x] 1.3 `kernel::purge::tombstone`, written in the purge job's transaction with the row's delete
- [x] 1.4 `workspaces.purge_workspace` purges the kernel's rows in batches, then deletes the row and writes the tombstone; its log line holds the id and the time only

## 2. The sweep

- [x] 2.1 `kernel.sweep_purged_workspaces`, hourly: the batched purge per tombstone; pending and running deliveries survive
- [x] 2.2 Tombstones older than 30 days with nothing left are dropped

## 3. Purging again after a restore

- [x] 3.1 `workspaces::operator::purge_again`: re-marks the workspace due now (deleting and publishing when it was live), enqueues the purge under its unique key
- [x] 3.2 `campbooks-server workspaces purge-again <id> --confirm <id>`; refuses without the same id
- [x] 3.3 The self-hosting guide's runbook

## 4. Tests

- [x] 4.1 A 12,000-row workspace takes three batches; another workspace is untouched; a second run is idempotent
- [x] 4.2 The sweep deletes leftovers, keeps a pending and a running delivery with their events, and leaves a live workspace alone
- [x] 4.3 Tombstones are dropped only when old and empty; one row per workspace; isolated by workspace
- [x] 4.4 The purge writes the tombstone, and the sweep then removes the purge's own finished job
- [x] 4.5 `purge_again` on a restored live workspace, on one in its grace period, twice, and on a purged id
- [x] 4.6 The purge line's fields; the command's confirmation
