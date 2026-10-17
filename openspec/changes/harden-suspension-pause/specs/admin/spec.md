## MODIFIED Requirements

### Requirement: Suspending an account
The `identity` module SHALL serve `POST /admin/v1/identity/accounts/{account_id}/suspend`
and `…/unsuspend` (`users.suspend`). Suspending SHALL, in one transaction,
mark the account suspended, end its issuer sessions, revoke its refresh-token
families, record `account_suspended` (with the administrator's account id)
in the account's audit trail and publish `identity.account_suspended`; the
web BFF SHALL then delete the account's web sessions. Unsuspending SHALL, in
one transaction, lift the suspension, record `account_unsuspended` and
publish `identity.account_unsuspended`. Each change SHALL add one to the
account's count of suspensions and lifts under the account's row lock, and
both events SHALL carry that count as `seq`, so a later change to one
account always carries a higher `seq` whatever the clock says, and `at`, the
moment it took effect, read under the same lock. Both operations SHALL be
idempotent: repeating one changes, counts, records and publishes nothing
more and answers the account as it is. Suspending an account that has an
admin profile SHALL be refused with 409 `account_is_admin`, read through the
kernel's `AdminProfiles` port, and change nothing.

#### Scenario: Suspend once, retry safely
- **WHEN** a super admin suspends an account twice
- **THEN** both answers are 200 with the same `suspended_at`, the account has no live session or refresh family, and one audit row and one event exist

#### Scenario: Suspend, then lift
- **WHEN** a super admin suspends an account and then unsuspends it twice
- **THEN** one `identity.account_unsuspended` is published, with an `at` later than the suspension's

#### Scenario: Counted, not timed
- **WHEN** an account is suspended, lifted, lifted again and suspended again
- **THEN** the three events published carry `seq` 1, 2 and 3, and the account's count is 3

#### Scenario: An administrator cannot be suspended over HTTP
- **WHEN** a super admin suspends an account that has an admin profile, asking in Portuguese
- **THEN** the answer is 409 `account_is_admin` whose detail tells, in Portuguese, to revoke the profile with the server's command line first, and the account is not suspended

#### Scenario: Web sessions end at once
- **WHEN** `identity.account_suspended` is delivered for an account signed in on two browsers
- **THEN** both browsers' web sessions are gone and another account's are not

### Requirement: Administrators can inspect and recover the job queue
The admin API SHALL let an administrator with `jobs.read` see the background
job queue as counts per queue and kind (ready, scheduled, running, running
past its lease, failed, age of the oldest ready job) and list the failed jobs
newest first with their kind, queue, workspace, attempts, last error and
times, filtered by kind or queue and paged by cursor. The counts SHALL also
list, for each code under which job kinds are watched (a kind's `Watch`),
those kinds and how many of their jobs are dead, 0 when none; when a job of
a watched kind dies the worker SHALL log, at WARN, the line its kind
declares, which holds no id or content. The suspension's deliveries
(`workspaces.announce_member_suspended`,
`workspaces.announce_member_unsuspended`,
`conversations.pause_suspended_author`,
`conversations.resume_unsuspended_author`) SHALL be watched as
`suspension_fanout`, logging `suspension fan-out delivery died`. An
administrator with
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

#### Scenario: A watched delivery dies
- **WHEN** a delivery of a watched kind fails for good
- **THEN** the worker logs its kind's line at WARN, and the queue's counts list its code with that kind and one dead job, while a dead job of an unwatched kind is not counted there

#### Scenario: Watches that disagree
- **WHEN** two kinds are watched under one code with different lines, or a code is not lower-case snake case
- **THEN** the registry refuses to build, naming each

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

### Requirement: The Jobs panel
The admin dashboard SHALL have a Jobs panel showing the work to watch (each
watched code named in words, "Suspensions reaching members' rules" for
`suspension_fanout`, with its kinds and either "No dead deliveries" or how
many died), the queue by kind and the
failed jobs with their last error in monospace, wrapped and copyable, and
SHALL let an administrator retry a job, discard it after a confirmation, and
retry every failed job of a kind after a confirmation. Every action SHALL say
what happened, and the panel SHALL work at 375 px, in light and dark, and in
the four shipped locales. The last error shown SHALL be the sanitized,
bounded message the worker stored, never a handler's raw message.

#### Scenario: Discarding asks first
- **WHEN** an administrator chooses Discard on a failed job
- **THEN** the panel asks for confirmation naming the kind and says the job is deleted for good, and nothing is deleted until they confirm

#### Scenario: Suspensions that never reached the rules
- **WHEN** two deliveries watched as `suspension_fanout` are dead
- **THEN** the panel's "Work to watch" lists "Suspensions reaching members' rules" with "2 dead deliveries"
