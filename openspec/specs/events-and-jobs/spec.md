# events-and-jobs Specification

## Purpose
How Campbooks records domain events and runs background work durably in
Postgres (ADR 0011, ADR 0012): events written with the change they describe
and delivered to every consumer, jobs retried, shared fairly between
workspaces and recovered from crashed workers, recurring schedules, replay,
and the process roles that run it all.

## Requirements

### Requirement: Transactional event publishing
Publishing a domain event SHALL happen inside the caller's database
transaction. It SHALL append the event (name, schema version, workspace,
payload, time) to the retained event log and SHALL enqueue one delivery per
consumer registered for that event name, in the same transaction. If the
transaction rolls back, neither the event nor any delivery SHALL exist. Every
event type SHALL declare an explicit schema version of at least 1.

#### Scenario: Rolled-back publish leaves nothing
- **WHEN** a module publishes an event in a transaction and then rolls the transaction back
- **THEN** the event log and the job queue contain nothing from it

#### Scenario: Fan-out to every consumer
- **WHEN** an event of workspace `ws-1` is published and committed, and two consumers are registered for its name
- **THEN** the log holds the event with its name, version and workspace, and the queue holds one pending delivery per consumer, each in workspace `ws-1`
- **AND** a worker runs both consumers with that event

#### Scenario: No consumers yet
- **WHEN** an event with no registered consumer is published
- **THEN** it is logged and no job is enqueued

### Requirement: Event delivery
A delivery SHALL hand its consumer the event's id, version, workspace, time
and payload. A delivery whose logged event has another name or schema
version than the consumer reads SHALL fail without retrying.

#### Scenario: Version mismatch
- **WHEN** a logged event's version differs from the version its consumer's event type declares
- **THEN** the delivery is marked failed with an error naming both versions

### Requirement: Replay
An operator SHALL be able to re-deliver logged events to one named consumer
with `campbooks-server events replay <consumer>`, optionally from an instant
(RFC 3339) or from an event id (inclusive) given as `--since`. Replay SHALL
enqueue nothing for events that already have an unfinished delivery to that
consumer, SHALL not affect other consumers, and SHALL report how many
deliveries it enqueued. An unknown consumer name SHALL be an error.

#### Scenario: Replaying one consumer
- **WHEN** three events were delivered to consumers A and B, and the operator replays A from the beginning
- **THEN** three new deliveries to A are enqueued and none to B

#### Scenario: Replay twice
- **WHEN** the operator replays the same consumer again before the first replay's deliveries ran
- **THEN** nothing more is enqueued

#### Scenario: Unknown consumer
- **WHEN** the operator replays a name no module registers
- **THEN** the command exits non-zero naming the consumer

### Requirement: Job enqueueing
A job SHALL be enqueued inside the caller's transaction, so it exists for
workers only once that transaction commits. A job MAY carry a workspace, a
priority, an earliest run time, a unique key and an ordering key. While a job
of the same kind with the same unique key is pending or running, enqueueing
another SHALL be a no-op reported as a duplicate; once it has finished, the
key SHALL be usable again.

#### Scenario: Duplicate unique key
- **WHEN** a job with unique key `mailbox-1` is pending and another job of the same kind with that key is enqueued
- **THEN** the second enqueue reports a duplicate and no job is added

#### Scenario: Key free after completion
- **WHEN** the job with unique key `mailbox-1` has succeeded and another is enqueued with that key
- **THEN** a new job is created

### Requirement: Serial execution per ordering key
Jobs sharing an ordering key SHALL run one at a time, in the order they were
enqueued. A job SHALL not be claimed while an earlier job with its ordering
key is pending or running; a failed (dead) job SHALL not block its key.

#### Scenario: One at a time
- **WHEN** three jobs share ordering key `thread-1` and a worker with free slots claims
- **THEN** only the first is claimed, and the second only after the first has finished

#### Scenario: Order kept under concurrency
- **WHEN** a worker with eight slots runs five jobs sharing an ordering key
- **THEN** they complete in enqueue order

### Requirement: Fair claiming across workspaces
A worker SHALL claim jobs so that no workspace starves another: in each
queue, at most `CAMPBOOKS_JOBS__PER_WORKSPACE` jobs of one workspace SHALL be
running at once (checked when claiming), and a claim SHALL take every
eligible workspace's next job before any workspace's second, preferring the
workspace served least recently. Two workers SHALL never claim the same job.

#### Scenario: Busy workspace, quiet workspace
- **WHEN** workspace A has 1,000 ready jobs, workspace B has 1 ready job enqueued after them, and a worker claims one job at a time
- **THEN** B's job is claimed by the second claim at the latest

#### Scenario: One claim, every workspace
- **WHEN** the same two workspaces are ready and a worker claims two jobs
- **THEN** it claims one job of each

#### Scenario: Per-workspace cap
- **WHEN** the cap is 2 and a workspace has 10 ready jobs and none running
- **THEN** a claim takes at most 2 of them, and no more until one finishes

#### Scenario: Concurrent workers
- **WHEN** two workers claim from the same 200 ready jobs at the same time, repeatedly
- **THEN** every job is claimed exactly once

### Requirement: Job execution in the workspace's transaction
A job handler SHALL get a transaction in which `campbooks.workspace_id` is set
to the job's workspace (and unset for instance-wide jobs). Its writes in that
transaction SHALL commit together with the job being marked succeeded, and
SHALL roll back if the handler fails or if the worker no longer holds the
job's lease.

#### Scenario: Workspace setting
- **WHEN** a job of workspace `ws-42` reads `current_setting('campbooks.workspace_id', true)` in its transaction
- **THEN** it reads `ws-42`

#### Scenario: Lost lease
- **WHEN** a job's lease is taken over by another worker while its handler runs
- **THEN** the handler's writes are rolled back and the job is not marked succeeded by the first worker

### Requirement: Retries and dead jobs
A handler failure marked retryable SHALL put the job back to pending with an
exponential backoff from `CAMPBOOKS_JOBS__RETRY_BASE_MS`, capped at
`CAMPBOOKS_JOBS__RETRY_MAX_SECONDS`, with 50–100 % jitter, or after the delay
the handler asks for. A failure marked fatal, a failure on the last attempt,
an undecodable payload or a kind no module registers SHALL mark the job
failed, keep it, and record the error truncated to 2,000 characters. A
panicking handler SHALL count as a retryable failure.

#### Scenario: Retry then success
- **WHEN** a job fails retryably on its first attempt with a 60 s base
- **THEN** it is pending with attempt 1, the error recorded, and a run time 30–60 s later, and its writes are rolled back
- **AND** once due it runs again and succeeds with attempt 2

#### Scenario: Fatal failure
- **WHEN** a handler returns a fatal error
- **THEN** the job is marked failed after one attempt with the error kept

#### Scenario: Attempts run out
- **WHEN** a job with three attempts fails retryably every time
- **THEN** it is retried twice and then marked failed

### Requirement: Leases and recovery
A claimed job SHALL be leased to its worker for `CAMPBOOKS_JOBS__LEASE_SECONDS`,
renewed while it runs. Workers SHALL return jobs whose lease expired to
pending (or failed, if that was their last attempt), so another worker runs
them.

#### Scenario: Worker died
- **WHEN** a running job's lease expires without renewal
- **THEN** the next upkeep pass makes it pending with a `lease expired` error, another worker runs it, and the original worker can no longer complete it

### Requirement: Recurring schedules
A module SHALL be able to schedule a job at a fixed interval of at least one
second, on ticks aligned to the Unix epoch. However many workers run, each
tick SHALL be enqueued at most once, and a tick SHALL be skipped while the
schedule's previous run is still pending or running. The kernel SHALL
schedule `kernel.prune_jobs` hourly, which deletes succeeded jobs older than
`CAMPBOOKS_JOBS__RETENTION_HOURS` and keeps failed jobs and the event log.

#### Scenario: Two workers, one tick
- **WHEN** two workers check schedules at the same time, several times within one tick
- **THEN** the scheduled job is enqueued once
- **AND** after it ran, checking again within the same tick enqueues nothing

#### Scenario: Pruning
- **WHEN** the prune runs while a succeeded and a failed job finished 30 days ago and another succeeded job finished just now
- **THEN** only the old succeeded job is deleted

### Requirement: Registration rules
Each module SHALL declare its jobs, event consumers and schedules in its
module descriptor, each named `<module>.<name>` in lower-case snake case and
unique. The server SHALL refuse to start, and `cargo xtask arch` SHALL fail,
on a misnamed or duplicate registration, a queue name that is not lower-case
snake case, zero attempts, or a schedule more frequent than once a second.

#### Scenario: Misnamed job
- **WHEN** module `test` registers a job named `other.misnamed`
- **THEN** building the registry fails naming the job and the expected `test.<name>` form

### Requirement: Kernel schema ownership
The event log and job queue SHALL live in Postgres schema `kernel`, created by
migrations named `<timestamp>_kernel__<description>.sql`. `cargo xtask arch`
SHALL fail when a migration creates, alters, drops or qualifies a name with a
schema other than its owner's (a module's own schema, or `kernel` for kernel
migrations), when a migration sets `search_path`, or when a module is named
`kernel`.

#### Scenario: Module migration touching the queue
- **WHEN** a `system` migration inserts into `kernel.jobs`
- **THEN** `cargo xtask arch` fails naming the file and the `kernel` schema

### Requirement: Process roles
`campbooks-server` SHALL run as `all` (the HTTP API and the job worker in one
process; the default when no command is given, and the image's command),
`api` (only the HTTP API) or `worker` (only the job worker); `serve` SHALL
remain an alias of `all`. Idle workers SHALL be woken by enqueue
notifications and SHALL otherwise poll every
`CAMPBOOKS_JOBS__POLL_INTERVAL_MS`. A role that runs the worker SHALL refuse
to start, naming the setting, when a `CAMPBOOKS_JOBS__*` value is out of
range.

#### Scenario: Low latency without hot polling
- **WHEN** a worker with a 10-minute poll interval is idle and a job is enqueued
- **THEN** the job runs within two seconds

#### Scenario: Invalid concurrency
- **WHEN** the worker starts with `CAMPBOOKS_JOBS__CONCURRENCY=0`
- **THEN** it exits with an error naming `CAMPBOOKS_JOBS__CONCURRENCY`

### Requirement: Graceful worker shutdown
On Ctrl-C or SIGTERM a worker SHALL stop claiming, give running jobs
`CAMPBOOKS_JOBS__SHUTDOWN_GRACE_SECONDS` to finish, then stop the rest and
return them to pending without counting the interrupted attempt.

#### Scenario: Quick and slow jobs at shutdown
- **WHEN** a worker with a 1 s grace period is stopped while a 300 ms job and a 60 s job run
- **THEN** the short job succeeds and the long one is pending again with no attempt counted and no lease

### Requirement: Job observability
Each job attempt SHALL run in a tracing span carrying the job id, kind, queue,
attempt and workspace, and SHALL log its outcome. Workers SHALL log their
counters (claimed, succeeded, retried, failed, released, lost) periodically
and when they stop. Job payloads and error messages SHALL not carry secrets.

#### Scenario: A job's log line
- **WHEN** a job succeeds
- **THEN** its log line carries the job's kind, queue, attempt and workspace, and the elapsed time
