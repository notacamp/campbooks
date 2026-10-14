## MODIFIED Requirements

### Requirement: Errors, logs and job errors carry no content
Provider error bodies SHALL NOT be kept in AI errors. Errors stored in
`kernel.jobs.last_error` or logged for a request's internal error SHALL
describe a database error by SQLSTATE, constraint and table only, and a JSON
error by category, line and column only, never the values they quote. A
stored message document that fails to decode SHALL be reported the same way.
The job worker SHALL store and log a failed job's message only after
sanitizing it: its first line only, quoted text replaced, email addresses
replaced, URLs cut to scheme and host, opaque token-like runs of 40 or more
characters replaced, control characters turned into spaces, and at most 500
characters.

#### Scenario: Provider echoes the mail
- **WHEN** an AI provider answers 400 with a body quoting the message's subject and sender
- **THEN** the error, its log lines and the job's `last_error` contain the status but neither the subject nor the address

#### Scenario: Unique violation on an address
- **WHEN** an internal error is a Postgres unique violation whose detail quotes an email address
- **THEN** the log line names `23505`, the constraint and the table, and not the address

#### Scenario: A handler quotes mail into its error
- **WHEN** a job handler fails with a message that quotes a provider reply holding a subject, an address, a link with a token and the mail's body on later lines
- **THEN** the job's `last_error` keeps the handler's own wording and the link's host, and contains neither the subject, the address, the token nor the body
