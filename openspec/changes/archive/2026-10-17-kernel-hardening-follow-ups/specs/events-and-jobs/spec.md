## ADDED Requirements

### Requirement: Instance-only job kinds
A job kind SHALL be instance-only when it is a schedule's, when it delivers
an instance-wide event type (`DomainEvent::WORKSPACE_SCOPED` false), or when
its job type declares `INSTANCE_ONLY`. The worker SHALL refuse a claimed job
of an instance-only kind that carries a workspace: it SHALL mark it failed
for good after one attempt, with an error naming the kind, and SHALL NOT run
its handler. The same kind's instance-wide jobs SHALL run as before. The
registry SHALL list the instance-only kinds.

#### Scenario: A prune written by hand in a workspace
- **WHEN** a transaction working in workspace A inserts, as raw SQL, a `kernel.prune_jobs` job of its own workspace with no schedule key (which row-level security admits), and a worker runs
- **THEN** the job is failed after one attempt with an error saying the kind runs instance-wide only, and nothing is pruned
- **AND** the schedule's next tick enqueues the instance-wide prune, which runs and prunes

#### Scenario: The registry lists them
- **WHEN** the registry is built from a module with a job, consumers of a workspace event and of an instance-wide event, and the kernel's schedules
- **THEN** its instance-only kinds are the kernel's schedules and the instance-wide event's consumer

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
schedule's run is instance-wide, so only another instance-wide row holding
its key counts as its previous run. A schedule's last tick recorded more
than one period ahead SHALL count as the current tick, so the schedule
runs. The current tick SHALL be read from the clock when the tick is
recorded, not when its transaction began, so a tick transaction that
stalled for more than a period SHALL NOT move a newer recorded tick back. The kernel SHALL schedule `kernel.prune_jobs` hourly, which deletes
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

#### Scenario: A stalled tick
- **WHEN** one worker's tick transaction begins and stalls for more than two periods while another worker records a newer tick, and the stalled one then records its tick
- **THEN** the recorded tick does not move back and the stalled worker enqueues nothing for a tick already run

#### Scenario: Pruning
- **WHEN** the prune runs while a succeeded and a failed job finished 30 days ago and another succeeded job finished just now
- **THEN** only the old succeeded job is deleted
