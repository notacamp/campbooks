# content-flows Specification

## Purpose
Where people's mail may go and what may be kept of it outside its own
records: AI providers, errors, logs, HTTP caches, the kernel's history, and
screen recordings.

## Requirements

### Requirement: No mail content to an AI endpoint off the instance's network without opt-in
The system SHALL classify each configured AI role's endpoint as on the
instance's network when its host is a loopback or private address
(`10/8`, `172.16/12`, `192.168/16`, `fc00::/7`), `localhost` or a
`.localhost` name, a single-label name, or a name listed in
`CAMPBOOKS_AI__INTERNAL_HOSTS` (exactly, or under a `.domain` entry), and as
external otherwise, without consulting DNS. The AI gateway SHALL NOT follow
HTTP redirects: a 3xx answer is a provider error, and nothing is sent to the
redirect target. The system SHALL NOT send mail
content (chat, embeddings or decide requests) to an external role unless
`CAMPBOOKS_AI__<ROLE>__EXTERNAL_CONTENT=allow`; such a call SHALL fail with
the problem code `ai_external_content_refused` before any request is made.
Health probes, which carry no content, SHALL run for every configured role.

#### Scenario: Hosted decide role without opt-in
- **WHEN** `CAMPBOOKS_AI__DECIDE__BASE_URL` is `https://decide.example.com/v1` and `CAMPBOOKS_AI__DECIDE__EXTERNAL_CONTENT` is unset
- **THEN** no request is sent to it for new mail, the model pass is skipped, and the rules alone sort the inbox

#### Scenario: Compose service
- **WHEN** a role's base URL is `http://laya:8000/v1`
- **THEN** it is on the instance's network and receives content without an opt-in

#### Scenario: Opted in
- **WHEN** `CAMPBOOKS_AI__COMPLETE__BASE_URL` is a hosted endpoint and `CAMPBOOKS_AI__COMPLETE__EXTERNAL_CONTENT=allow`
- **THEN** calls of the `complete` role are sent to it

#### Scenario: On-network endpoint redirects elsewhere
- **WHEN** a role's on-network endpoint answers 307 or 308 pointing at another host
- **THEN** the call fails with that status and the other host receives nothing

#### Scenario: Stray opt-in
- **WHEN** `CAMPBOOKS_AI__COMPLETE__EXTERNAL_CONTENT=allow` is set but the role has no base URL and no model
- **THEN** the server starts and the role is not configured

### Requirement: Administrators see where each AI role is
The admin system overview SHALL report, for each configured AI role, whether
its endpoint is on the instance's network (`location`: `instance` or
`external`) and whether it may receive mail content (`content_allowed`). The
admin screen SHALL say so in words in every supported language.

#### Scenario: External role not opted in
- **WHEN** an administrator opens Admin → System and the `complete` role points off the instance's network without opt-in
- **THEN** the role reads "Not sent any mail" and says it is outside this instance's network

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

### Requirement: API responses are not cached
Every response under `/api/`, `/bff/` and `/admin/` SHALL carry
`Cache-Control: no-store` (or a handler's own `no-store` variant) and
`Pragma: no-cache`. The web client's own files SHALL keep their caching.

#### Scenario: Reading a conversation
- **WHEN** a client reads any `/api/v1/...` or `/bff/api/...` resource
- **THEN** the response carries `Cache-Control: no-store` and `Pragma: no-cache`

### Requirement: Kernel history is bounded
The system SHALL delete events older than `CAMPBOOKS_EVENTS__RETENTION_DAYS`
(default 90) except events a pending or running job still delivers, failed
jobs older than `CAMPBOOKS_JOBS__FAILED_RETENTION_DAYS` (default 30), and
succeeded jobs older than `CAMPBOOKS_JOBS__RETENTION_HOURS` (default 168), by
an hourly maintenance job. Purging a deleted workspace SHALL delete its
events, finished jobs and change feed, and SHALL keep its unfinished jobs.
Each retention below one SHALL stop the server at startup naming the setting.

#### Scenario: Old delivered event
- **WHEN** an event is 120 days old and every delivery of it has finished
- **THEN** the next prune deletes it

#### Scenario: Old undelivered event
- **WHEN** an event is 120 days old and a delivery of it is still pending
- **THEN** it is kept

#### Scenario: Workspace purged
- **WHEN** a deleted workspace is purged at the end of its grace period
- **THEN** no event and no finished job of that workspace remains

### Requirement: Recordings are made on this machine only
The recording tool SHALL refuse a base URL whose host is not `localhost`,
`127.0.0.1` or `[::1]` unless `--allow-remote` is passed, and SHALL refuse a
URL that is not http(s) in every case.

#### Scenario: Production URL
- **WHEN** someone runs `pnpm record inbox --base-url https://app.example.com`
- **THEN** the tool exits without opening a browser and says it records this machine only
