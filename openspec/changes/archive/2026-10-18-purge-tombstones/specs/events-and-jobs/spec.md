## ADDED Requirements

### Requirement: Sweeping purged workspaces
The kernel SHALL schedule `kernel.sweep_purged_workspaces` hourly. For every
tombstone of a purged workspace it SHALL delete, whatever their age, the
workspace's changes and cursor head, finished jobs, and events no pending or
running delivery reads. It SHALL work 5,000 rows per table per transaction,
in a transaction working in that workspace. A pending or running delivery,
and the event it reads, SHALL survive.

A tombstoned workspace that is live (its row is back and not deleted) SHALL
be skipped: nothing of it is deleted, its tombstone stays, and one warning
names its id only. A workspace whose check or purge fails SHALL be logged
and skipped, its tombstone kept, and the sweep SHALL go on to the others.

A tombstone SHALL be dropped once it is older than 30 days and nothing of
its workspace is left in the queue, the event log or the change feed.
Tombstones SHALL carry the standard workspace-isolation policy of the
kernel's tables: a transaction working in one workspace SHALL NOT write
another workspace's tombstone.

#### Scenario: Leftovers of a purged workspace
- **WHEN** a tombstoned workspace has a finished delivery, a failed job, a change and its head, and a pending and a running delivery, all recent, and the sweep runs
- **THEN** only the pending and running deliveries and their events are left, and a live workspace's recent rows are untouched

#### Scenario: A tombstoned workspace that is live
- **WHEN** the sweep runs over tombstones of a live workspace with rows, a live one with none, one whose check fails, and a purged one, all older than 30 days
- **THEN** nothing of the first three is deleted and their tombstones stay, while the purged one's rows and tombstone go

#### Scenario: Dropping tombstones
- **WHEN** the sweep runs over a 31-day-old tombstone with nothing left, a 31-day-old one with a pending delivery, and a day-old one with nothing left
- **THEN** only the first is dropped

#### Scenario: Another workspace's tombstone
- **WHEN** a transaction working in one workspace inserts a tombstone for another, as raw SQL
- **THEN** row-level security refuses it
