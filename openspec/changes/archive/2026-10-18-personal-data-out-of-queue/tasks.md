## 1. The rule and its checks

- [x] 1.1 AGENTS.md: personal data stays out of event payloads, job payloads and job errors
- [x] 1.2 `xtask` `payloads`: event payload fields named like personal data hold opaque values or are listed with a reason
- [x] 1.3 Every job payload field is listed with its reason (read from `impl Job` / `impl JobHandler` and the payload struct)
- [x] 1.4 Every value a job error is built from is listed per file with its reason; stale entries reported in all three lists
- [x] 1.5 Wired into `cargo xtask arch`

## 2. Errors that quoted data

- [x] 2.1 Payload and event decode errors in the queue: `redact::json`
- [x] 2.2 Connectors' cursor, batch, document and requested-change errors: `redact::json`
- [x] 2.3 Identity prune: `redact::sqlx`; member-left publishes: through `From<sqlx::Error>`

## 3. Tests

- [x] 3.1 The personal-name matcher; event fields (opaque kinds, nested, allow-listed, stale)
- [x] 3.2 Job payloads read from sources (unit, handler with a constant name, attributes, comments and tests skipped); missing and stale audit entries
- [x] 3.3 Job error values (literals, inline and positional and named arguments, strings with commas, `Self::` only in `JobError`'s file); missing and stale entries

## 4. Review round 1

- [x] 4.1 `BlobError::Database` names a database error by kind; `Self::…` in any `impl From<…> for JobError` is scanned
- [x] 4.2 Job error values carry their number of uses per file: a new use fails until reviewed
- [x] 4.3 `JobError` constructors passed as functions fail; variants written out are scanned (patterns are not)
- [x] 4.4 A job whose name or payload cannot be read, an ambiguous payload struct, and a name constant with several values are reported
- [x] 4.5 Every free-text event field is listed by its full path (`*` for a path in every event)
- [x] 4.6 Gmail's raw message decode error uses `provider::unreadable`
