## MODIFIED Requirements

### Requirement: Workspace deletion
An owner SHALL be able to delete a workspace. Deletion SHALL mark it deleted,
publish `WorkspaceDeleting` with the time it will be purged, and schedule
its purge after a 30-day grace period, in one transaction. A deleted
workspace SHALL answer 404 and SHALL disappear from members' lists at once.
The purge SHALL remove the workspace with its members, invitations and
security events. It SHALL delete the workspace's kernel rows (change feed,
finished jobs, events no pending or running delivery reads) at most 5,000
per table per transaction, then delete the workspace's row and record its
tombstone (its id and the time, nothing else) in one transaction. Its log
line SHALL carry the workspace's id and the time only.

#### Scenario: Delete then purge
- **WHEN** an owner deletes a workspace
- **THEN** `workspaces.workspace_deleting` is in the event log, reads of the workspace answer 404, and a `workspaces.purge_workspace` job is scheduled 30 days later
- **AND** when that job runs, the workspace's rows are gone and its tombstone is recorded

#### Scenario: A large workspace
- **WHEN** a workspace with 12,000 delivered events is purged
- **THEN** its kernel rows go in three transactions, and another workspace's rows stay

#### Scenario: The purge's log line
- **WHEN** a workspace is purged
- **THEN** the `workspace purged` line's fields are the message, the workspace's id and the time

## ADDED Requirements

### Requirement: Purging a workspace again after a restore
`campbooks-server workspaces purge-again <id> --confirm <id>` SHALL purge a
workspace a restored backup brought back. It SHALL refuse, reading and
changing nothing, unless `--confirm` repeats the id exactly. An id with no
workspace SHALL be a no-op that says so.

For a workspace deleted in the restored database:
- if its purge is queued, the command SHALL change nothing and SHALL report
  when the purge runs;
- otherwise it SHALL set the purge time to now and SHALL queue the purge.

For a live workspace:
- without `--not-deleted-in-backup`, the command SHALL refuse, changing
  nothing, and SHALL print only facts: that it is live, its member count and
  its creation date;
- with `--not-deleted-in-backup`, it SHALL delete the workspace at once
  (hidden from its members), set its purge time to an hour from now, or to
  `--purge-after` (refused when less than an hour away), publish
  `WorkspaceDeleting`, and queue the purge for that time. The event SHALL
  name the workspace's first owner as who deleted it, a stand-in for the
  operator. A live workspace with no owner SHALL be refused, changing
  nothing.

The purge SHALL be queued under its unique key, so a purge already queued
means no second one. The command SHALL log one line with the workspace's id
only when it changes something. Setting a purge time sooner than the app's
grace period SHALL be possible through this command only: no API route or
admin endpoint SHALL do it.

#### Scenario: A workspace live in the restored database
- **WHEN** the operator runs `purge-again` for a live workspace without `--not-deleted-in-backup`
- **THEN** the command fails, saying it is live with its member count and creation date, and nothing is published, queued or changed

#### Scenario: A workspace deleted after the backup was taken
- **WHEN** the operator runs `purge-again --not-deleted-in-backup` twice for a live workspace, and a worker runs once its purge time has passed
- **THEN** the first run hides it, publishes its deletion naming its first owner, and queues its purge an hour later; the second reports the queued purge's time and enqueues none; the worker purges it
- **AND** a later run says there is no such workspace and enqueues nothing

#### Scenario: A purge time given
- **WHEN** the operator gives `--purge-after` half an hour away, then twelve days away
- **THEN** the first is refused and nothing changes, and the second is the workspace's purge time

#### Scenario: No owner
- **WHEN** the operator runs `purge-again --not-deleted-in-backup` for a live workspace with no owner
- **THEN** the command fails and nothing changes

#### Scenario: A workspace in its grace period
- **WHEN** the operator runs `purge-again` for a workspace deleted through the app
- **THEN** its purge time and its one queued purge stay, the command reports when the purge runs, and its deletion is not published again

#### Scenario: A deleted workspace whose purge gave up
- **WHEN** the operator runs `purge-again` for a deleted workspace with no purge queued
- **THEN** its purge time becomes now, a purge is queued, and a worker purges it

#### Scenario: No confirmation
- **WHEN** the operator runs `purge-again <id>` without `--confirm`, or with another id
- **THEN** the command fails and nothing is read or changed
