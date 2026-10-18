## ADDED Requirements

### Requirement: Personal data stays out of the queue and the event log
Event payloads, job payloads and job errors (`kernel.jobs.last_error`) SHALL
hold ids, codes, counts and blob references, never addresses, names,
subjects or what someone wrote; such data SHALL travel as a blob.

`cargo xtask arch` SHALL check three things, and SHALL report an entry in
any of its three lists that no longer matches anything:
- every field of every event payload in the catalogue, at any depth, named
  like personal data, SHALL hold an opaque value (a blob reference, a UUID,
  an enum, a number, a flag, or a list or nullable of such) or be listed in
  `EVENT_FIELDS_ALLOWED` with its reason;
- every field of every job kind's payload SHALL be listed in
  `JOB_PAYLOAD_FIELDS` with its reason;
- every value a job error message is built from, other than its own wording,
  SHALL be listed for its file in `JOB_ERROR_VALUES` with its reason.

A job error built from a JSON or database error SHALL name it by kind
(`redact::json`, `redact::sqlx`), never by its message.

#### Scenario: An event field named like personal data
- **WHEN** an event payload carries `subject` as a plain string, not listed
- **THEN** `cargo xtask arch` fails naming the event and the field, and passes once it is a blob reference or listed with a reason

#### Scenario: A new job payload field
- **WHEN** a job's payload struct gains a field no audit entry lists
- **THEN** `cargo xtask arch` fails naming the job and the field

#### Scenario: A job error built from a new value
- **WHEN** a job error interpolates a value no audit entry lists for its file
- **THEN** `cargo xtask arch` fails naming the file and the value

#### Scenario: A stale entry
- **WHEN** a list names an event field, a payload field or an error value that no longer exists
- **THEN** `cargo xtask arch` fails asking for the entry to be removed
