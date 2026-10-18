## Why

The event log keeps payloads for 90 days, and finished jobs keep payloads
and failure reasons (`kernel.jobs.last_error`) for up to 30. That is longer
than a member's or a workspace's data once its deletion is asked for
(campbooks-cloud#65, PR 3).

Today the payloads hold ids and blob references by convention only, and job
errors rely on `redact::job_error`'s masking as their only guard. Several
job errors still printed a JSON or database error as it was. A JSON error
can quote the value it failed on, and a database error's message can quote
a row's.

## What Changes

- **A rule** (AGENTS.md, security rules): event payloads, job payloads and
  job errors hold ids, codes, counts and blob references, never addresses,
  names, subjects or what someone wrote.
- **`cargo xtask arch` enforces it** (`xtask/src/payloads.rs`):
  - **Event payloads:** every field of every event in the catalogue is read,
    at any depth. A field named like personal data must hold an opaque value
    (a blob reference, a UUID, an enum, a number, a flag, or a list or
    nullable of such), or be listed in `EVENT_FIELDS_ALLOWED` with its
    reason. The provider and identity subjects and the provider message ids
    are listed.
  - **Job payloads:** every field of every job kind's payload, read from
    the sources (`impl Job` / `impl JobHandler`), is listed in
    `JOB_PAYLOAD_FIELDS` with its reason, field by field.
  - **Job errors:** every value a `JobError::fatal` / `retry` /
    `retry_after` message is built from, other than its own wording, is
    listed per file in `JOB_ERROR_VALUES` with its reason.
  - A stale entry in any list is reported.
- **Errors that quoted data now say what failed by kind:**
  - the queue's payload and event decode errors, and the connectors'
    cursor, batch and document errors, use `redact::json`;
  - the identity prune's database errors use `redact::sqlx`;
  - the identity member-left handler's publish errors go through
    `JobError`'s `From<sqlx::Error>`, which redacts.

## Impact

- Spec: events-and-jobs (Personal data stays out of the queue and the event
  log).
- Code:
  - `xtask` (`payloads`, wired into `arch`);
  - kernel `jobs::registry`;
  - connectors `send_message`, `sync_stream` and `request_changes`;
  - identity `prune` and `member_left`.
- No migration, no API change.
