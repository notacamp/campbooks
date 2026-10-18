## Why

Purging a deleted workspace left pieces of it behind (campbooks-cloud#65):

- **Rows written after the purge.** Another module's purge, still queued
  when the workspace's own purge ran, publishes events, and its job (and the
  workspace purge's own job) finishes afterwards. Those rows of a workspace
  that no longer exists stayed until normal retention: up to 90 days for
  events, 30 for failed jobs.
- **One long transaction.** The kernel's rows of a workspace (its change
  feed, event log and finished jobs) were deleted by single statements in
  the purge job's transaction. A large workspace could hold locks for a long
  time or run past the statement timeout and never be purged.
- **A restore brings purged workspaces back.** A backup taken before a
  purge holds the workspace, its members and its data. The operator had no
  way to purge it again but to delete it through the app and wait another
  30 days.

## What Changes

- **Tombstones.** A new kernel table, `kernel.purged_workspaces` (the
  workspace's id and the time, nothing else), gets a row in the same
  transaction that deletes the workspace's row. It carries the standard
  kernel `workspace_isolation` policy and the runtime role's usual grant: no
  new role, no new grant of another kind, no row-level-security exception.
- **Batched purge.** The purge deletes the workspace's kernel rows 5,000
  per table at a time, each batch in a short transaction of its own working
  in the workspace, before deleting the workspace's row.
- **The sweep.** A new hourly kernel schedule,
  `kernel.sweep_purged_workspaces`, runs the same batched purge for every
  tombstone. A delivery still pending or running, and the event it reads,
  survive. A tombstone is dropped once nothing of its workspace is left and
  it is older than 30 days.
- **Purge again after a restore.** `campbooks-server workspaces purge-again
  <id> --confirm <id>` marks a restored workspace deleted with its purge due
  now and enqueues the purge. If the backup predates the deletion, it also
  publishes the deletion so every module purges its data. A purge already
  queued means no second one, and an id with no workspace is a no-op that
  says so. It runs from the server binary only: there is no route and no
  admin endpoint.
- **The purge's log line** carries only the workspace's id and the time, so
  it can serve as the list of workspaces to purge again after a restore.
- **Self-hosting:** the guide's "Backups" section gains the runbook.

## Impact

- Specs: workspaces (Workspace deletion; Purging again after a restore),
  events-and-jobs (Sweeping purged workspaces).
- Code:
  - kernel `purge` (new module, with `SweepPurgedWorkspaces`);
  - the batched `jobs::purge_workspace` and `changes::purge_workspace`;
  - the registry's schedule;
  - the workspaces module's `PurgeWorkspace` and `purge_again`;
  - the server's `workspaces purge-again` command.
- Migration `20261022050000_kernel__purged_workspaces`.
- Prod's restore runbook: campbooks-cloud#261.
