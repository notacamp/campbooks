## MODIFIED Requirements

### Requirement: Job enqueueing
A job SHALL be enqueued inside the caller's transaction, so it exists for
workers only once that transaction commits. A job MAY carry a workspace, a
priority, an earliest run time, a unique key and an ordering key. While a job
of the same kind with the same unique key is pending or running in the same
workspace, enqueueing another SHALL be a no-op reported as a duplicate; once
it has finished, the key SHALL be usable again. Unique keys SHALL count per
workspace, the instance-wide jobs (no workspace) forming one more group: a
job of another workspace, or an instance-wide one, with the same kind and
key SHALL NOT make an enqueue a duplicate.

#### Scenario: Duplicate unique key
- **WHEN** a job with unique key `mailbox-1` is pending and another job of the same kind with that key is enqueued in the same workspace
- **THEN** the second enqueue reports a duplicate and no job is added

#### Scenario: Key free after completion
- **WHEN** the job with unique key `mailbox-1` has succeeded and another is enqueued with that key
- **THEN** a new job is created

#### Scenario: The same key in two workspaces
- **WHEN** workspaces A and B, and instance-wide work, each enqueue a job of the same kind with unique key `connectors.sync_stream:s-1`, each twice
- **THEN** the first enqueue of each is created and the second is a duplicate, and the three jobs run

#### Scenario: Holding another workspace's key
- **WHEN** a transaction working in workspace B inserts, as raw SQL, a job of its own holding workspace A's unique and ordering key and due never, and workspace A then enqueues a job with that key
- **THEN** A's job is created and runs, and B's row stays pending

### Requirement: Serial execution per ordering key
Jobs of one workspace sharing an ordering key SHALL run one at a time, in
the order they were enqueued. A job SHALL not be claimed while an earlier
job of its workspace with its ordering key is pending or running; a failed
(dead) job SHALL not block its key. Ordering keys SHALL count per
workspace, the instance-wide jobs forming one more group: a job SHALL NOT
wait for another workspace's jobs, and an enqueuer SHALL NOT wait for
another workspace's enqueuer, with the same ordering key.

#### Scenario: One at a time
- **WHEN** three jobs of one workspace share ordering key `thread-1` and a worker with free slots claims
- **THEN** only the first is claimed, and the second only after the first has finished

#### Scenario: Order kept under concurrency
- **WHEN** a worker with eight slots runs five jobs sharing an ordering key
- **THEN** they complete in enqueue order

#### Scenario: The same ordering key in two workspaces
- **WHEN** workspace A has two jobs ordered by `thread-1`, workspace B one, instance-wide work one, and a worker with free slots claims
- **THEN** A's first, B's and the instance-wide job are claimed together, and A's second only after A's first has finished

#### Scenario: Enqueuers of two workspaces
- **WHEN** a transaction of workspace A has enqueued a job ordered by `thread-1` and not yet ended, and workspace B enqueues a job ordered by `thread-1`
- **THEN** B's enqueue completes without waiting for A's transaction

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
schedule's run is instance-wide, so only another instance-wide row holding
its key counts as its previous run. A schedule's last tick recorded more
than one period ahead SHALL count as the current tick, so the schedule
runs. The kernel SHALL schedule `kernel.prune_jobs` hourly, which deletes
succeeded jobs older than `CAMPBOOKS_JOBS__RETENTION_HOURS` and keeps
failed jobs and the event log.

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
