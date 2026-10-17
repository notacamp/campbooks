## MODIFIED Requirements

### Requirement: Recurring schedules
A module SHALL be able to schedule a job at a fixed interval of at least one
second, on ticks aligned to the Unix epoch. However many workers run, each
tick SHALL be enqueued at most once, and a tick SHALL be skipped while the
schedule's previous run is still pending or running. A row holding the
schedule's unique key that would never run as the schedule's (one of a
workspace, one pending and due later than twice the longer of the
schedule's period and the longest retry delay, one running on a lease
ending later than twice the worker's lease) SHALL NOT block it: the next
tick SHALL mark that row failed, with the reason, and enqueue afresh. A
schedule's last tick recorded more than one period ahead SHALL count as the
current tick, so the schedule runs. The
kernel SHALL schedule `kernel.prune_jobs` hourly, which deletes succeeded
jobs older than `CAMPBOOKS_JOBS__RETENTION_HOURS` and keeps failed jobs and
the event log.

#### Scenario: Two workers, one tick
- **WHEN** two workers check schedules at the same time, several times within one tick
- **THEN** the scheduled job is enqueued once
- **AND** after it ran, checking again within the same tick enqueues nothing

#### Scenario: A stuck schedule row
- **WHEN** rows holding schedules' keys exist from before the policies refused them (one never due, one of a workspace, one leased forever), next to one waiting out a retry and one held by a live worker, and a worker checks the schedules
- **THEN** the three stuck rows are marked failed with the reason and each schedule has a fresh pending run, while the retrying and the live rows are left as they were

#### Scenario: A last tick far ahead
- **WHEN** a schedule's recorded last tick is the largest number there is and a worker checks the schedules
- **THEN** the schedule's job is enqueued and its last tick is the current one

#### Scenario: Pruning
- **WHEN** the prune runs while a succeeded and a failed job finished 30 days ago and another succeeded job finished just now
- **THEN** only the old succeeded job is deleted
