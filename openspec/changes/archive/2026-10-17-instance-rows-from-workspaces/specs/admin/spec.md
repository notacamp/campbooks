## MODIFIED Requirements

### Requirement: Administrators can inspect and recover the job queue
The admin API SHALL let an administrator with `jobs.read` see the background
job queue as counts per queue and kind (ready, scheduled, running, running
past its lease, failed, age of the oldest ready job) and list the failed jobs
newest first with their kind, queue, workspace, attempts, last error and
times, filtered by kind or queue and paged by cursor. An administrator with
`jobs.retry` SHALL be able to put one failed job, or the failed jobs of one
kind (at most 1000 per call), back on their queue with their attempts reset,
and to delete one failed job. Such an administrator SHALL also be able to
discard a schedule's pending or stuck run by the schedule's name (its row is
pending, belongs to a workspace, or runs on a lease that expired or is
longer than any worker gives), so the schedule's next tick enqueues afresh;
a run a live worker holds SHALL be kept, and an unknown name SHALL answer
404. No response SHALL contain a job's payload.

#### Scenario: A member or a read-only administrator
- **WHEN** someone without an admin profile calls any jobs operation, or an administrator holding only `jobs.read` calls retry or discard
- **THEN** the response is 404 and nothing changes

#### Scenario: Failed jobs without payloads
- **WHEN** an administrator lists the failed jobs and one of them carries a payload with free text
- **THEN** the response lists its kind, queue, attempts and last error and contains no trace of the payload

#### Scenario: Retrying a failed job
- **WHEN** an administrator retries a failed job
- **THEN** the job is pending again, due now, with zero attempts, an idle worker is woken, and the worker runs it again

#### Scenario: Only failed jobs are touched
- **WHEN** an administrator retries or discards a job that is pending, running or succeeded
- **THEN** the job is left as it was and the answer says nothing changed

#### Scenario: Repeating is harmless
- **WHEN** an administrator retries the same failed job twice, or discards it twice
- **THEN** the second call succeeds and changes nothing

#### Scenario: A retry never double-runs
- **WHEN** failed jobs are retried while several workers are claiming
- **THEN** every job runs exactly once per retry

#### Scenario: A unique key already waiting
- **WHEN** a failed job's unique key is held by another pending or running job of the same kind
- **THEN** retrying it changes nothing and the answer says it was superseded

#### Scenario: Discarding a schedule's stuck run
- **WHEN** an administrator with `jobs.retry` discards the run of `kernel.prune_jobs` while a row due never holds its key, then again, and then discards the run of a schedule a live worker is running
- **THEN** the first call deletes the row and is audited, the second changes nothing, and the live run is kept; an administrator with only `jobs.read` gets 404 and a name that is no schedule gets 404

