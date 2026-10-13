## Purpose

Who belongs to which workspace and with what role: users keyed by their
identity-provider subject, workspaces, members, invitations, the instance's
signup policy, the security log and workspace deletion. Also how every
request resolves its workspace, and how Postgres row-level security keeps
each workspace's rows apart (ADR 0011).

## ADDED Requirements

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
- **THEN** the response is 403 with detail `signup_invite_only`

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
- **THEN** the response is 409 with detail `last_owner` and nothing changes

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
`member`. The API SHALL never return the invitation token; Campbooks SHALL
store only a hash of it. An invitation SHALL expire after 7 days and SHALL
be usable once. Accepting SHALL require a user token whose email is
verified and equal, ignoring case, to the invited address. When the server
runs with `CAMPBOOKS_ENV=development` it SHALL log the accept link; in any
other environment it SHALL NOT.

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
