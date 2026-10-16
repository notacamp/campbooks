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
`mail=keep`. `workspaces.member_left` SHALL carry the choice. Keeping
SHALL be refused with 409 `keep_needs_a_reader` when an inbox the member
connected would have nobody left to read it, asked in the departure's own
transaction. A member who leaves on their own SHALL always have their mail
deleted. Accepting an invitation SHALL be refused with 409 `still_leaving`
while a departure of that person from the workspace is not yet handled by
every module.

#### Scenario: Offboarding with history kept
- **WHEN** an owner removes a member with `mail=keep`
- **THEN** `workspaces.member_left` says `"mail": "keep"`

#### Scenario: Keeping mail nobody could read
- **WHEN** an owner removes, with `mail=keep`, a member who is the only member of an inbox they connected
- **THEN** the answer is 409 `keep_needs_a_reader` and the member stays

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

### Requirement: Unverified addresses are not shown as members'
The workspaces module SHALL record whether the identity provider verified a
user's address at their last sign-in. A member's address SHALL be listed
(`workspaces.list_members`, `workspaces.change_role`) only when verified, and
inviting an address SHALL treat it as a member's only when a member verified
it.

#### Scenario: A claimed address
- **WHEN** a person whose provider did not verify `ceo@example.com` creates a workspace and its members are listed
- **THEN** their entry has no address, and after a sign-in where the provider verifies it the address is listed

### Requirement: Users from the identity provider
A user SHALL be identified by their token's subject. The first time a user
calls a user-level operation (list workspaces, create a workspace, accept an
invitation), Campbooks SHALL create their user record from the token's
subject, name and email and SHALL record a `signed_in` security event. Later
calls SHALL update a changed name or email and SHALL NOT create another
record. A user SHALL be able to belong to any number of workspaces.

#### Scenario: First sign-in
- **WHEN** a user whose subject Campbooks has never seen lists their workspaces
- **THEN** a user record is created with that subject, name and email, a `signed_in` security event is recorded, and the list is empty

#### Scenario: Same subject again
- **WHEN** the same user calls again with a new email in their token
- **THEN** the same user record is reused and its email is updated

### Requirement: Workspace resolution
Workspace-owned operations SHALL take the workspace from the path
(`/api/v1/w/{workspace_id}/…`). After the token and scope checks, the
request SHALL be refused with 404 when the id is not a UUID, when the
workspace does not exist or is deleted, when a user is not a member, or
when a client token's workspace claim is missing or names another
workspace.

#### Scenario: Not a member
- **WHEN** a signed-in user with the right scope reads a workspace they are not a member of
- **THEN** the response is 404 problem+json, the same as for a workspace that does not exist

#### Scenario: Client token bound to its workspace
- **WHEN** a client token whose `workspace_id` claim is workspace A reads workspace A and then workspace B
- **THEN** the read of A succeeds and the read of B is 404

#### Scenario: Token checked first
- **WHEN** a request without a token calls a workspace-owned operation
- **THEN** the response is 401, whatever the workspace id

### Requirement: Row-level security
Every workspace-owned table SHALL have Postgres row-level security enabled
and forced, with a policy that admits only rows of the workspace set in
`campbooks.workspace_id` for the transaction. The server SHALL run queries as
a role subject to those policies, and SHALL set the workspace on the
transaction of every workspace-owned request and job.

#### Scenario: Cross-workspace read blocked in SQL
- **WHEN** a transaction running as the runtime role sets `campbooks.workspace_id` to workspace B and selects workspace A's members, invitations or security events with no filter
- **THEN** no row of workspace A is returned

#### Scenario: No workspace set
- **WHEN** a query on a workspace-owned table runs as the runtime role with no workspace set
- **THEN** it returns no row and does not fail

### Requirement: Creating a workspace
A signed-in user SHALL be able to create a workspace with a name (1–100
characters), a kind (`business` or `personal`) and optionally an accent
(`business`, `personal` or `forest`; default the kind's). The creator SHALL
become its owner. `WorkspaceCreated` and `MemberJoined` SHALL be published
in the same transaction. Client tokens SHALL NOT create workspaces.

#### Scenario: Create
- **WHEN** a user creates workspace "Household" of kind `personal`
- **THEN** it is returned with accent `personal` and the user's role `owner`, it appears in their workspace list, and `workspaces.workspace_created` and `workspaces.member_joined` are in the event log

### Requirement: Signup policy
When a user who is not a member of any workspace creates one, the instance's
signup policy SHALL apply: `open` allows it; `invite_only` refuses it with
403 `signup_invite_only`; `approval` refuses it with 403
`signup_requires_approval` and records a `signup_requested` security event;
`code` allows it only with a `signup_code` equal to one of the configured
codes, else 403 `signup_code_invalid`. The instance's first user SHALL
always be allowed. A user who is already a member of a workspace SHALL be
allowed under every policy.

#### Scenario: Invite-only instance
- **WHEN** the policy is `invite_only` and a second user with no workspace tries to create one
- **THEN** the response is 403 with code `signup_invite_only`

#### Scenario: Bootstrap
- **WHEN** the policy is `invite_only` and the instance's first user creates a workspace
- **THEN** it is created

#### Scenario: Signup code
- **WHEN** the policy is `code` and a new user sends a configured code
- **THEN** the workspace is created; with a wrong or missing code the response is 403 `signup_code_invalid`

### Requirement: Roles
Members SHALL have exactly one role: `owner`, `admin` or `member`. Owners
and admins SHALL be able to rename a workspace, change its accent, change
roles, remove members and invite. Only an owner SHALL grant or remove the
`owner` role, change an owner's role, remove an owner or delete the
workspace. A workspace SHALL always keep at least one owner. Role-gated
actions by a client token SHALL be refused with 403.

#### Scenario: Last owner
- **WHEN** the only owner demotes themself, removes themself or leaves
- **THEN** the response is 409 with code `last_owner` and nothing changes

#### Scenario: Admin cannot touch an owner
- **WHEN** an admin changes an owner's role or removes an owner
- **THEN** the response is 403 and nothing changes

#### Scenario: Role change published
- **WHEN** an owner makes a member an admin
- **THEN** `workspaces.member_role_changed` is published with the old and new role, and a `role_changed` security event is recorded

### Requirement: Leaving and removal
A member SHALL be able to leave a workspace, and owners and admins SHALL be
able to remove members, subject to the role rules. `MemberLeft` SHALL be
published and a `member_left` or `member_removed` security event recorded.

#### Scenario: Remove a member
- **WHEN** an admin removes a member
- **THEN** that user's reads of the workspace answer 404 and `workspaces.member_left` names who removed them

### Requirement: Invitations
Owners and admins SHALL be able to invite an email address as `admin` or
`member`. The API SHALL return the invitation's accept link (which carries
its token) only once, in the response that creates the invitation, and SHALL
never list, log or publish it in an event; Campbooks SHALL store only a hash
of the token. An invitation SHALL expire after 7 days and SHALL be usable
once. Accepting SHALL require a user token whose email is verified and
equal, ignoring case, to the invited address.

#### Scenario: Accept
- **WHEN** the invited user, with a verified matching email, accepts with the token
- **THEN** they become a member with the invited role, `workspaces.member_joined` is published and an `invitation_accepted` security event is recorded

#### Scenario: Single use
- **WHEN** a used token is accepted again
- **THEN** the response is 409 `invitation_used`

#### Scenario: Expired
- **WHEN** a token is accepted after its expiry
- **THEN** the response is 409 `invitation_expired` and no membership is created

#### Scenario: Wrong email
- **WHEN** a user whose email differs from the invited address, or is not verified, accepts the token
- **THEN** the response is 403 `invitation_email_mismatch` and the invitation stays usable

#### Scenario: Unknown token
- **WHEN** a token that was never issued is accepted
- **THEN** the response is 404

### Requirement: Workspace deletion
An owner SHALL be able to delete a workspace. Deletion SHALL mark it deleted,
publish `WorkspaceDeleting` with the time it will be purged, and schedule
its purge after a 30-day grace period, in one transaction. A deleted
workspace SHALL answer 404 and SHALL disappear from members' lists at once.
The purge SHALL remove the workspace with its members, invitations and
security events.

#### Scenario: Delete then purge
- **WHEN** an owner deletes a workspace
- **THEN** `workspaces.workspace_deleting` is in the event log, reads of the workspace answer 404, and a `workspaces.purge_workspace` job is scheduled 30 days later
- **AND** when that job runs, the workspace's rows are gone

### Requirement: Security log
Campbooks SHALL record security events — first sign-in, workspace created,
role changed, invitation created and accepted, member removed or left,
workspace deleted, signup requested — with the workspace, the user, the kind,
the time and metadata holding no secret. Owners and admins SHALL be able to
read their workspace's events, newest first.

#### Scenario: Reading the log
- **WHEN** an admin reads the security log after a role change
- **THEN** a `role_changed` event lists the user and the old and new role, and no token or email appears in any event

### Requirement: A suspension reaches every workspace of the account
On `identity.account_suspended` and `identity.account_unsuspended` the module
SHALL find the user whose identity subject is the account and publish, in
each workspace they are a member of that is not being deleted,
`workspaces.member_suspended` or `workspaces.member_unsuspended` with the
workspace, the user and the event's `at` (its `occurred_at` when it has
none), each in a transaction working in that workspace. An account with no
user, or a user in no workspace, SHALL publish nothing. Delivering an event
again MAY publish again; consumers SHALL treat the newest `at` as the truth.

#### Scenario: A member of two workspaces
- **WHEN** an account whose user belongs to two workspaces is suspended
- **THEN** `workspaces.member_suspended` is published once in each, with that user and the suspension's `at`, and nothing for another user

#### Scenario: Never signed in
- **WHEN** an account that never signed in (no user) is suspended
- **THEN** nothing is published
