## Context

`workspaces.purge_workspace` hard-deletes a workspace when its grace period
ends. It purges the workspace's blobs, its kernel rows (change feed, event
log, finished jobs) and its row, whose foreign keys cascade to its members,
invitations and security log. Every other module purges its own data from
its own job, scheduled by `WorkspaceDeleting` for the same time.

## Decisions

### Tombstones in the kernel, under the standard policy

The tombstone lives in `kernel.purged_workspaces (workspace_id, purged_at)`.
The purge job writes it in its own transaction, which works in the
workspace, so the table needs no exception: it carries the same
`workspace_isolation` policy as every workspace-keyed kernel table (the
kernel scope or the row's own workspace). The sweep reads the tombstones in
the kernel scope, then purges each workspace in a transaction working in
that workspace.

### The sweep is its own schedule, not a clause of the prunes

The retention prunes (`kernel.prune_jobs`, `kernel.prune_events`) walk their
age indexes. Adding "or the workspace is tombstoned" to their statements
would turn those into scans of the whole queue and log every hour. Instead
`kernel.sweep_purged_workspaces` runs the purge's own batched function per
tombstone, so the purge and the sweep share one tested path. That path keeps
pending and running deliveries and the events they read.

### Batches in short transactions, before the row's delete

The kernel's rows go first, 5,000 per table per transaction, then the
workspace row and the tombstone in the job's transaction. Rows that appear
between the last batch and the commit are the sweep's. If the job fails
after some batches, it retries, and the purge is idempotent.

### Tombstones do not survive a restore

The tombstones are in the same database as everything else, so a restored
backup holds the tombstones of its own moment, not those written after it.
They cannot tell which workspaces were purged after the backup.

**What they fix:** rows written after a purge, and, for a backup taken
after a purge, rows that reach the purged workspace afterwards.

**What fixes a restore:** the operator's runbook. Two log lines list the
workspaces to act on after the backup's time. A test pins the fields of
`workspace purged` to the id and the time. `workspace deleted` carries the
original `purge_after`, so it covers workspaces deleted after the backup but
not yet purged. `campbooks-server workspaces purge-again <id> --confirm <id>`
then handles each one:

- **A workspace deleted in the backup.** Its queued purge stays as it is;
  the command changes nothing and reports when the purge runs. If no purge
  is queued (it gave up), the command sets the purge time to now and queues
  a purge.
- **A workspace live in the backup.** By default the command refuses and
  prints only facts: that the workspace is live, its member count and its
  creation date. A mistyped id then costs nothing. With
  `--not-deleted-in-backup`:
  - the workspace is hidden at once and its deletion is published, so every
    module purges its data;
  - the event names its first owner, a stand-in for the operator, since
    `deleted_by` must be a member;
  - the purge comes an hour later, or at `--purge-after` (never sooner than
    an hour), which leaves time to undo a mistyped id through the restore.
- **A live workspace with no owner** is refused. Every workspace keeps an
  owner, so this needs a person to look at it, not a guess.
- **The purge is queued** under its unique key, so a purge already queued
  means no second one.
- **It stays with the operator.** It is a subcommand of the server binary,
  with no route or admin endpoint. It refuses unless `--confirm` repeats the
  id exactly, and it logs one line with the id only. Only this command sets
  a purge time sooner than the grace period.

### The sweep leaves a live workspace alone

A tombstone and a live row of the same workspace mean something went wrong:
a partial restore, or an id reused by hand. The sweep checks the membership
port's `workspace_live` before each purge. It skips a live workspace with
one warning naming its id, and keeps the tombstone. It also logs and skips
a workspace whose check or purge fails, and moves on to the next.

### The purge's log line is written before the commit

The worker commits the job's transaction together with marking the job
succeeded, after `run` returns. `JobContext` has no after-commit hook, and
adding one for a log line would change every job's contract. So
`workspace purged` is logged at the end of `run`, before that commit. If
the commit fails, the line names a workspace that is still there and still
deleted. `purge-again` on it then reports the queued purge and changes
nothing, so the runbook stays safe.

### Rejected: a purge ledger outside the database

An append-only record of purges kept in blob storage (under `_purged/`)
would survive a database restore only if blob storage were not restored to
the same moment. Backups take both together, and the guide asks for exactly
that. So the ledger would be rolled back too, or would disagree with the
database in ways nobody could check. The log line, kept by the log store
rather than by the backup, already gives the list.

## Risks

- **Logs kept for less time than the oldest backup** leave the operator
  with no list. The guide says to keep server logs at least as long as the
  oldest backup.
- **Module purges keep their own schedule.** A module's purge queued for a
  later time keeps that time when `purge-again` runs during a real grace
  period. After a restore, the time has normally passed already.
