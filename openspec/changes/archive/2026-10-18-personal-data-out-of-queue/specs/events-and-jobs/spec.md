## ADDED Requirements

### Requirement: Personal data stays out of the queue and the event log
Event payloads, job payloads and job errors (`kernel.jobs.last_error`) SHALL
hold ids, codes, counts and blob references, never addresses, names,
subjects or what someone wrote; such data SHALL travel as a blob.

`cargo xtask arch` SHALL check three things, and SHALL report an entry in
any of its three lists that no longer matches anything:
- every field of every event payload in the catalogue, at any depth, that
  can hold free text (a string that is not an id, a date or one of a closed
  set, outside a blob reference) SHALL be listed by its full path in
  `EVENT_FIELDS_ALLOWED` with its reason;
- every field of every job kind's payload SHALL be listed in
  `JOB_PAYLOAD_FIELDS` with its reason, and a job whose name or payload
  cannot be read SHALL be reported;
- every value a job error message is built from, other than its own wording,
  SHALL be listed for its file with its number of uses in `JOB_ERROR_VALUES`
  with its reason, including messages built in an `impl From<…> for
  JobError` or by writing a variant out, and a `JobError` constructor passed
  as a function SHALL be reported.

A job error built from a JSON or database error SHALL name it by kind
(`redact::json`, `redact::sqlx`), never by its message, including through an
error enum's message.

#### Scenario: A free-text event field
- **WHEN** an event payload carries `items[].subject` as a plain string, not listed by that path
- **THEN** `cargo xtask arch` fails naming the event and the path, and passes once it is a blob reference, an id or listed with a reason

#### Scenario: A new job payload field
- **WHEN** a job's payload struct gains a field no audit entry lists
- **THEN** `cargo xtask arch` fails naming the job and the field

#### Scenario: A job error built from a new value
- **WHEN** a job error interpolates a value no audit entry lists for its file, or one more time than listed
- **THEN** `cargo xtask arch` fails naming the file and the value

#### Scenario: A stale entry
- **WHEN** a list names an event field, a payload field or an error value that no longer exists
- **THEN** `cargo xtask arch` fails asking for the entry to be removed
