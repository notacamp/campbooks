# api-rate-limits Specification

## Purpose
Per-caller limits on the authenticated API, so a stolen token, a buggy app
or a looping automation rule cannot hammer the instance or send mail as fast
as the provider allows, while a person using the app never meets them.

## Requirements

### Requirement: Every authenticated operation is rate-limited per caller
Every operation that requires a bearer scope SHALL be counted against a
per-caller limit before its handler runs, whether it is called over HTTP,
through the web BFF (`/bff/api/*`, `/bff/admin/*`) or in-process by
automation or the assistant (`operations::invoke`). The admin API SHALL be
counted the same way. Public operations SHALL not be counted by this limit,
and a request whose token does not verify SHALL not be counted (it is
answered 401 as before). Over the limit the answer SHALL be 429 problem+json
with `code` `rate_limited`, a `Retry-After` header in whole seconds (at
least 1) saying when the next request is allowed, and `title`/`detail` in the
caller's language; the handler SHALL not run and no idempotency key SHALL be
claimed. A replayed idempotent request SHALL count as a hit.

#### Scenario: A person over the read limit
- **WHEN** a person makes more reads in a minute than the read limit allows
- **THEN** the next read answers 429 `rate_limited` with `Retry-After`, and once that many seconds have passed reads succeed again

#### Scenario: Through the web app
- **WHEN** a signed-in person's browser calls `/bff/api/*` more often than the read limit allows
- **THEN** the BFF's call answers 429 `rate_limited`, counted against the same person and web app

#### Scenario: A token that does not verify
- **WHEN** a request carries an expired token
- **THEN** it is answered 401 as before and counts against nobody

### Requirement: Classes of operations
Each operation SHALL be counted in one class, decided by the operation
catalogue: **read** for an operation whose `x-campbooks.mutates` is false
(including reads served by POST), **write** for a mutating one, **send**
for an operation whose `x-campbooks.rate_limit` is `send`
(`conversations.send_draft`), and **admin** for every admin API operation.
Each class SHALL have its own limit, so exhausting writes or sends SHALL not
refuse reads.

#### Scenario: Writes run out first
- **WHEN** a person makes more writes in a minute than the write limit allows
- **THEN** the next write answers 429, while a read still succeeds

#### Scenario: Sends run out before writes
- **WHEN** a person sends more drafts in the send window than the send limit allows
- **THEN** the next send answers 429 `rate_limited`, while other writes and cancelling a scheduled send still succeed

### Requirement: Who shares a budget
Reads, writes and admin calls SHALL be counted per person and app: a
person's token SHALL be counted under its subject together with the client it
was issued to (`azp`), and a `client_credentials` client under its client
id. Sends SHALL be counted per person across all their apps (per client id
for a client). Two people SHALL never share a budget, and a client SHALL
never share a person's.

#### Scenario: Two people
- **WHEN** one person has exhausted the read limit
- **THEN** another person's reads still succeed

#### Scenario: Two apps of one person
- **WHEN** a person's third-party app has exhausted the read limit
- **THEN** the same person's reads through the web app still succeed

#### Scenario: A client
- **WHEN** a person has exhausted the read limit
- **THEN** a workspace's `client_credentials` client still reads, counted under its own client id

### Requirement: Automation and the assistant are limited separately
Operations invoked in-process by automation or by the assistant on behalf of
a person SHALL be counted per (person, caller kind) at the same read and
write limits, apart from the person's own apps, so a looping rule or
assistant is stopped without refusing the person's own requests. Neither
SHALL send: `conversations.send_draft` opts neither in, so an invoked send
is refused before it is counted, and the person's send budget is theirs
alone.

#### Scenario: A looping rule
- **WHEN** automation invokes reads for a person beyond the read limit
- **THEN** the invocation fails with 429 `rate_limited`, while the person's own reads through the web app still succeed

#### Scenario: A send by the assistant
- **WHEN** the assistant or automation invokes `conversations.send_draft` for a person, as a call marked confirmed
- **THEN** the invocation is refused before the handler runs, and the person's own sends through the web app still have their whole budget

### Requirement: Limits are configurable
The limits SHALL be read from `CAMPBOOKS_RATE_LIMITS__*`: `ENABLED`
(default `true`; `false` turns every per-caller limit off) and, per class, a
count and a window in seconds (`READS`=600/`READS_WINDOW_SECONDS`=60,
`WRITES`=120/`WRITES_WINDOW_SECONDS`=60, `SENDS`=30/`SENDS_WINDOW_SECONDS`=600,
`ADMIN`=120/`ADMIN_WINDOW_SECONDS`=60). A count of 0 SHALL turn that class
off. A window outside 1 second to 1 day SHALL stop startup with a message
naming the setting. Reads, writes and admin calls SHALL be counted in each
server process's memory (a token bucket allowing a burst of the count, then
one per window/count, with memory bounded by sweeping full buckets);
sends SHALL be counted in Postgres, shared by every process, with buckets
stored only as an HMAC under the instance secret.

#### Scenario: Turned off
- **WHEN** `CAMPBOOKS_RATE_LIMITS__ENABLED=false`
- **THEN** no request is refused by a per-caller limit, however many are made

#### Scenario: One class off
- **WHEN** `CAMPBOOKS_RATE_LIMITS__WRITES=0`
- **THEN** writes are never refused by the write limit, while reads still are by theirs

#### Scenario: A bad window
- **WHEN** `CAMPBOOKS_RATE_LIMITS__SENDS_WINDOW_SECONDS=0`
- **THEN** the server refuses to start, naming `CAMPBOOKS_RATE_LIMITS__SENDS_WINDOW_SECONDS`

### Requirement: Refusals are observable without personal data
When a caller's budget starts being refused, the server SHALL log one line
naming the class, the principal kind, the caller kind (app, automation,
assistant) and the operation id, and SHALL count every refusal in the metric
`campbooks.rate_limit.refusals` by class, principal kind and caller kind.
Neither SHALL carry a token, a subject, a client id, an address or any
content.

#### Scenario: A script hammers the API
- **WHEN** a script keeps calling after its first 429
- **THEN** one log line records that it was limited, and the refusal counter grows with every refused call
