## ADDED Requirements

### Requirement: Sweeping purged workspaces
The kernel SHALL schedule `kernel.sweep_purged_workspaces` hourly. For every
tombstone of a purged workspace it SHALL delete, whatever their age, the
workspace's changes and cursor head, finished jobs and events no pending or
running delivery reads. It SHALL work 5,000 rows per table per transaction,
in a transaction working in that workspace. A pending or running delivery
and the event it reads SHALL survive. A tombstone SHALL be dropped once it
is older than 30 days and nothing of its workspace is left in the queue, the
event log or the change feed. Tombstones SHALL carry the standard
workspace-isolation policy of the kernel's tables.

#### Scenario: Leftovers of a purged workspace
- **WHEN** a tombstoned workspace has a finished delivery, a failed job, a change and its head, and a pending and a running delivery, all recent, and the sweep runs
- **THEN** only the pending and running deliveries and their events are left, and a live workspace's recent rows are untouched

#### Scenario: Dropping tombstones
- **WHEN** the sweep runs over a 31-day-old tombstone with nothing left, a 31-day-old one with a pending delivery, and a day-old one with nothing left
- **THEN** only the first is dropped
