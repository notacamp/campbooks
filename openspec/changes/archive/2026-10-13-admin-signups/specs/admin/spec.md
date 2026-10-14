## ADDED Requirements

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
