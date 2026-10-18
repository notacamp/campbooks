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
workspace a restored backup brought back. It SHALL refuse, changing
nothing, unless `--confirm` repeats the id exactly. It SHALL set the
workspace's purge time to now. A workspace that is not deleted SHALL also be
deleted now, with `WorkspaceDeleting` published (its first owner as who
deleted it). It SHALL enqueue the purge under the purge's unique key, so a
purge already queued means no second one. An id with no workspace SHALL be a
no-op that says so. It SHALL log one line with the workspace's id only.
Setting the purge time to now SHALL be possible through this command only:
no API route or admin endpoint SHALL do it.

#### Scenario: A workspace restored from before its deletion
- **WHEN** the operator runs `purge-again` twice for a live workspace, then a worker runs
- **THEN** the first run deletes it and publishes its deletion, the second enqueues no second purge, and the worker purges it
- **AND** a third run says there is no such workspace and enqueues nothing

#### Scenario: A workspace in its grace period
- **WHEN** the operator runs `purge-again` for a workspace deleted through the app
- **THEN** its purge time is now, its one queued purge stays, and its deletion is not published again

#### Scenario: No confirmation
- **WHEN** the operator runs `purge-again <id>` without `--confirm`, or with another id
- **THEN** the command fails and nothing is read or changed
