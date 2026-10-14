# admin Specification

## Purpose
Who administers an instance and how: admin profiles granted only from the
server's CLI, a separate admin API outside the operation catalogue, its
authorisation, audit trail and cross-workspace reads, and the admin
dashboard with its System panel (ADR 0015).

## Requirements

### Requirement: Admin profiles exist only through the server CLI
An instance administrator SHALL be a user whose identity account has an admin
profile. Profiles SHALL be created, changed, listed and revoked only by the
server binary's `admin grant|revoke|list` subcommands, which run on the
migration login and record each change in the admin audit trail. No HTTP
endpoint SHALL read, create, change or delete an admin profile, and the
runtime database role SHALL NOT be able to write one.

#### Scenario: Granting from the CLI
- **WHEN** an operator runs `campbooks-server admin grant --email ada@example.com --permission super --yes` and that address is a verified email or way in of exactly one account
- **THEN** the account gets a profile with `super`, the command prints the account id and permissions, and the change is recorded with `via = cli`

#### Scenario: Ambiguous or unknown account
- **WHEN** the email matches no account, or more than one
- **THEN** the command exits non-zero and changes nothing; `--account <id>` is required to disambiguate

#### Scenario: Runtime role cannot write
- **WHEN** a query on the runtime login tries to insert, update or delete a row in `admin.profiles`
- **THEN** Postgres refuses it with a permission error

#### Scenario: CI keeps profile endpoints out
- **WHEN** a change adds a route to the `admin` module, an operation owned by `admin` in either OpenAPI document, or Rust source outside the `admin` module and the server CLI that references schema `admin`
- **THEN** `cargo xtask arch` fails naming the offending file or operation

### Requirement: Admin permissions
Admin permissions SHALL be a closed set with stable names (`super`,
`system.read`, `signups.review`, `users.read`, `users.suspend`,
`workspaces.read`, `jobs.read`, `jobs.retry`); `super` SHALL imply every
permission. Permissions SHALL NOT appear in tokens; they SHALL be read from
the profile on every admin request, so a revocation takes effect on the next
request.

#### Scenario: Super implies all
- **WHEN** an account whose profile holds only `super` calls an operation that requires `system.read`
- **THEN** the call is allowed

#### Scenario: Revocation is immediate
- **WHEN** an operator revokes a profile while its owner has a valid session
- **THEN** that owner's next admin request answers 404

### Requirement: Admin API authorisation
Admin operations SHALL be served under `/admin/v1/<module>/` and SHALL
require a user (not client-credentials) bearer token carrying the scope
`admin:access` and an admin profile holding the operation's permission;
otherwise they SHALL answer 404, indistinguishable from an unknown route.
Once those pass, a token whose `auth_time` is more than 12 hours old SHALL be
refused with 401 and the problem code `admin_reauth_required`. The issuer
SHALL grant `admin:access` only to first-party clients.

#### Scenario: Not an administrator
- **WHEN** a signed-in user without a profile calls `GET /admin/v1/system/overview`
- **THEN** the response is 404 and nothing is recorded in the audit trail

#### Scenario: Third-party client
- **WHEN** an OAuth client that is not first-party requests the scope `admin:access`
- **THEN** the issuer does not grant it, and an admin request with that client's token answers 404

#### Scenario: Stale authentication
- **WHEN** a super admin's token has `auth_time` 13 hours ago
- **THEN** admin requests answer 401 with code `admin_reauth_required`, and after signing in again they succeed

### Requirement: Admin API is outside the operation catalogue
Admin operations SHALL be described in a separate generated document,
`openapi/admin.json`, and SHALL NOT appear in `openapi/openapi.json`; the
kernel's invoke-by-id, and therefore the assistant, automation and MCP, SHALL
NOT be able to call them. Each admin operation SHALL declare an `operationId`
owned by the module that serves it and an `x-campbooks-admin` block naming its
permission.

#### Scenario: Invoke refuses admin operations
- **WHEN** `kernel::operations::invoke` is asked to run `system.admin_overview`
- **THEN** it answers that the operation does not exist

#### Scenario: Document kept current
- **WHEN** an admin route changes and `openapi/admin.json` is not regenerated
- **THEN** `cargo xtask openapi --check` fails

### Requirement: Admin requests are audited
Every admin request that passes the profile check SHALL be recorded in
`admin.actions` with the actor's account id, `operationId`, path parameters,
response status and time. The trail SHALL be append-only for the runtime
role.

#### Scenario: Read recorded
- **WHEN** a super admin opens the System panel
- **THEN** a row `(actor, 'api', 'system.admin_overview', {}, 200)` is appended

### Requirement: Cross-workspace reads are explicit and metadata-only
Admin handlers SHALL see other workspaces' rows only inside a transaction
opened with `kernel::db::begin_admin`, which requires the grant produced by
the admin extractor, and only on tables that carry an admin-read policy.
Content tables (messages and their bodies, attachments, documents) SHALL
never carry one. Only the kernel SHALL reference the setting that enables
admin reads.

#### Scenario: Content table gets an admin policy
- **WHEN** a migration adds an admin-read policy to a table outside the pinned allowlist
- **THEN** the server's test suite fails naming the table

### Requirement: Admin dashboard
The client SHALL offer an Admin section at `/admin`, outside any workspace,
reachable from the account menu only for users whose admin overview request
succeeds, in all four locales, at 375 px and desktop widths. When the API
answers `admin_reauth_required`, the section SHALL ask the user to sign in
again and return them to the page they were on.

#### Scenario: Entry hidden for members
- **WHEN** a user without a profile opens the account menu
- **THEN** there is no Admin item, and visiting `/admin` shows the not-found page

### Requirement: System panel
The system module SHALL expose `GET /admin/v1/system/overview` (permission
`system.read`) returning the version and commit, the latest applied migration
and the migration count, the process start time, the database's health, a
summary of the effective configuration that contains no secrets (signup
policy, configured sign-in providers, blob store kind, configured mail
providers), and per AI role whether it is configured, its model, its base URL
reduced to scheme and host, and whether it answered a probe within 3 seconds
(probe results cached for 60 seconds).

#### Scenario: No secrets
- **WHEN** an AI role's base URL contains credentials or the configuration holds API keys
- **THEN** the overview shows only the scheme and host and never any key or password

### Requirement: Administrators can inspect and recover the job queue
The admin API SHALL let an administrator with `jobs.read` see the background
job queue as counts per queue and kind (ready, scheduled, running, running
past its lease, failed, age of the oldest ready job) and list the failed jobs
newest first with their kind, queue, workspace, attempts, last error and
times, filtered by kind or queue and paged by cursor. An administrator with
`jobs.retry` SHALL be able to put one failed job, or the failed jobs of one
kind (at most 1000 per call), back on their queue with their attempts reset,
and to delete one failed job. No response SHALL contain a job's payload.

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

### Requirement: The Jobs panel
The admin dashboard SHALL have a Jobs panel showing the queue by kind and the
failed jobs with their last error in monospace, wrapped and copyable, and
SHALL let an administrator retry a job, discard it after a confirmation, and
retry every failed job of a kind after a confirmation. Every action SHALL say
what happened, and the panel SHALL work at 375 px, in light and dark, and in
the four shipped locales.

#### Scenario: Discarding asks first
- **WHEN** an administrator chooses Discard on a failed job
- **THEN** the panel asks for confirmation naming the kind and says the job is deleted for good, and nothing is deleted until they confirm

### Requirement: Signup administration
Administrators holding `signups.review` (or `super`) SHALL be able, through
the admin API, to list signup requests (filtered by status, pending first,
cursor-paged), approve or reject a request (a rejection may carry a reason of
at most 500 characters), list signup codes, mint a code and revoke a code.
Approving, rejecting and revoking SHALL be idempotent. Listing codes SHALL
never reveal a code or its digest; minting SHALL return the code in that
response only. Approving and rejecting SHALL be recorded in the security log.
Everyone else SHALL get 404.

#### Scenario: Deciding is idempotent
- **WHEN** an administrator approves the same request twice
- **THEN** both answer 200 with the same decision time and one `signup_approved` entry exists

#### Scenario: A code is shown once
- **WHEN** an administrator mints a code
- **THEN** the response contains the code, and neither the list of codes nor any later response does

#### Scenario: Not an administrator
- **WHEN** a member, or an administrator with only `jobs.read`, calls any signup operation
- **THEN** it answers 404

#### Scenario: Paging
- **WHEN** a list is read with `limit=2` and each response's `next` is passed as `after`
- **THEN** every row appears exactly once, pending requests first

### Requirement: Admin reads of signup requests are metadata only
`workspaces.signup_requests` SHALL be readable across users only inside an
admin transaction and SHALL hold only ids, status and times; the set of
tables with an admin-read policy SHALL remain pinned by a test.

#### Scenario: Without an admin transaction
- **WHEN** a query runs as the runtime role with no user and no admin setting
- **THEN** it sees no signup request
