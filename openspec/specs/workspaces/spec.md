# workspaces Specification

## Purpose
How tenancy is kept apart: workspaces, their members and the database
safeguards that keep one workspace's rows from another's (ADR 0011).

## Requirements

### Requirement: Runtime database login cannot bypass row-level security
The server SHALL run requests and jobs on a database login that is not a
superuser, does not have `BYPASSRLS`, is not a member of such a role and owns
no table with row-level security, and SHALL apply migrations on a separate
connection (`CAMPBOOKS_DATABASE_MIGRATION_URL` when set). A query on the
runtime login SHALL NOT be able to become a privileged role or see another
workspace's rows. At startup the server SHALL refuse to serve, naming the
reason, when the role it queries as can bypass row-level security or owns
policy tables, unless `CAMPBOOKS_DATABASE_ALLOW_RLS_BYPASS` is set.

#### Scenario: Dedicated login
- **WHEN** the runtime login is a plain member of `campbooks_app` and a query runs `SET ROLE` to a superuser or `RESET ROLE`
- **THEN** the first is refused, the second leaves the session on the same unprivileged login, and a workspace transaction sees only its own workspace's rows

#### Scenario: Privileged login
- **WHEN** the runtime URL is a superuser and no migration URL is set
- **THEN** the connections switch to `campbooks_app` and the server starts with a warning recommending separate logins

#### Scenario: Refusal
- **WHEN** the runtime role is a superuser (for example `CAMPBOOKS_DATABASE_ROLE` empty), or a migration URL is set and the runtime login is a superuser
- **THEN** the server refuses to start unless `CAMPBOOKS_DATABASE_ALLOW_RLS_BYPASS` is true

### Requirement: Removing a member decides their mail
`DELETE …/members/{user_id}` SHALL take `mail=delete` (the default) or
`mail=keep`. `workspaces.member_left` SHALL carry the choice. A member who
leaves on their own SHALL always have their mail deleted.

#### Scenario: Offboarding with history kept
- **WHEN** an owner removes a member with `mail=keep`
- **THEN** `workspaces.member_left` says `"mail": "keep"`
### Requirement: Signup requests under the approval policy
Under the `approval` signup policy, a user with no workspace who tries to
create one SHALL get exactly one signup request, however often they try, and
SHALL be refused with `signup_requires_approval` while it is pending. An
approved user SHALL be allowed to create their workspace. A rejected user
SHALL be refused with the problem code `signup_rejected`. A user SHALL be
able to read their own request status, and nobody else's, through
`GET /api/v1/signup`.

#### Scenario: Asking twice files one request
- **WHEN** a new user tries to create a workspace twice under the `approval` policy
- **THEN** both attempts answer 403 `signup_requires_approval`, one request and one `signup_requested` log entry exist, and `GET /api/v1/signup` shows `pending`

#### Scenario: Approved
- **WHEN** an administrator approves the request
- **THEN** the user's next attempt creates the workspace

#### Scenario: Rejected
- **WHEN** an administrator rejects the request
- **THEN** the user's attempts answer 403 `signup_rejected` and the reason the administrator wrote is not in the answer

#### Scenario: The first user and members are not held back
- **WHEN** the instance's first user, or a member of a workspace, creates a workspace under any policy
- **THEN** it is allowed

### Requirement: Minted signup codes are single-use and stored as digests
Under the `code` signup policy, a signup code SHALL be either one configured
in `CAMPBOOKS_SIGNUP__CODES` (reusable, the legacy path) or one minted by an
administrator. A minted code SHALL be stored only as its SHA-256 digest, with
a label, a number of allowed uses (default 1) and an optional expiry. A use
SHALL be spent in the same transaction that creates the workspace, atomically:
two attempts at the last use SHALL NOT both succeed, and a failed creation
SHALL NOT spend a use. A code that is unknown, spent, expired or revoked
SHALL be refused with `signup_code_invalid`. The `code` policy SHALL start
without any environment code.

#### Scenario: One use
- **WHEN** two new users present the same single-use code at the same moment
- **THEN** exactly one workspace is created and the other attempt answers 403 `signup_code_invalid`

#### Scenario: Forgiving typing
- **WHEN** a user types a minted code in lower case with spaces instead of dashes
- **THEN** it is accepted

#### Scenario: Environment codes keep working
- **WHEN** `CAMPBOOKS_SIGNUP__CODES` lists a code and several new users present it
- **THEN** each creates a workspace
