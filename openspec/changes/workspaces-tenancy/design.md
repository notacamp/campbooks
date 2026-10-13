# Design

## Context

The kernel validates access tokens (`Scoped<S>`, #433) and runs jobs whose
transaction carries `campbooks.workspace_id` (#434). Nothing yet knows what a
workspace is: client tokens carry a `workspace_id` claim nobody checks, and
users have no workspaces at all. See proposal.md for why this is needed now.

## Data ownership

Everything lives in schema `workspaces`, owned by the `workspaces` module.

| Table | Workspace-owned | Row-level security |
| --- | --- | --- |
| `users` (id, `idp_subject` unique, display name, email, locale `en`/`pt-PT`/`es-ES`/`fr`, created_at) | no — the instance's directory of people who signed in | none |
| `workspaces` (id, name, kind `business`/`personal`, accent `business`/`personal`/`forest`, created_at, deleted_at, purge_after) | yes (its `id`) | current workspace; members also see their own workspaces |
| `members` (workspace_id, user_id, role, joined_at; primary key on the pair) | yes | current workspace; a user also sees their own memberships |
| `invitations` (id, workspace_id, email, role, token_hash, expires_at, invited_by, created_at, accepted_at, accepted_by) | yes | current workspace |
| `security_events` (id, workspace_id nullable, user_id, kind, at, metadata jsonb) | yes, or the user's own when `workspace_id` is null | current workspace; a user also sees their own instance-level events |

All ids are UUIDv7 generated in Rust. Foreign keys stay inside the schema.
`metadata` holds ids and roles only, never tokens, emails or names.

## Row-level security

### Policies

Two transaction-local settings drive every policy:

- `campbooks.workspace_id` — the workspace a request or job works in; set by
  `PgPool::begin_for(workspace)` (kernel `db::BeginFor`) and by
  `JobContext::tx` (same setting, same helper).
- `campbooks.user_id` — the signed-in user, set by the module for the few
  user-level reads (my workspaces, my memberships).

Each workspace-owned table has `ENABLE` and `FORCE ROW LEVEL SECURITY` and a
policy `workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid`
(for `workspaces`, on `id`). `nullif` matters: once a connection has used the
setting, an unset value reads as `''` rather than NULL, and `''::uuid` would
raise instead of matching nothing. Without the setting, a query sees no row.
`FORCE` makes the policies apply to the tables' owner too, for deployments
where the runtime login owns the tables.

### Which role is subject to it

Superusers and `BYPASSRLS` roles skip row-level security, even with `FORCE`,
and the compose database user is a superuser. So the server does not query
as the user it connects with:

1. **Migrations** run first, on a plain connection as the configured user.
   The kernel migration creates the `campbooks_app` role (`NOLOGIN`, no
   superuser, no `BYPASSRLS`) when it does not exist, and makes the current
   user a member of it. Every migration grants `campbooks_app` exactly the
   privileges its own tables need (`USAGE` on the schema, DML on tables).
2. **The runtime pool** switches each new connection to that role
   (`set_config('role', 'campbooks_app', false)`, i.e. `SET ROLE`), so every
   request, job and event delivery runs as a role that row-level security
   applies to. `CAMPBOOKS_DATABASE_ROLE` names another role, or is set empty
   to skip the switch (e.g. when the login is already an unprivileged role
   that owns nothing, with `FORCE` covering an owner login).
3. At startup the server logs a warning if the pool's effective role still
   bypasses row-level security.

For self-hosters this is invisible: one database user, as today. A managed
Postgres whose migration user lacks `CREATEROLE` needs the role created once
by an administrator (`CREATE ROLE campbooks_app NOLOGIN; GRANT campbooks_app
TO <user>;`); the migration then only grants.

Tests that exercise the policies switch to the same role with
`kernel::testing::app_role_pool`, so the module's HTTP tests run under
row-level security exactly like production.

A server test asserts that every table with a `workspace_id` column (plus
`workspaces.workspaces`) has row-level security enabled and forced, and that
`campbooks_app` may use every table of every schema, so a future module
cannot forget either.

## Request flow: resolving the workspace

**Path, not header.** Workspace-owned resources live under
`/api/v1/w/{workspace_id}/<module>/…`; the workspace's own record is
`/api/v1/w/{workspace_id}` and its members and invitations sit directly under
it. Why the path:

- The workspace is part of the resource's identity. A URL names it, so links,
  bookmarks, logs and traces carry it, and two browser tabs can work in two
  workspaces with no client-side "current workspace" state.
- OpenAPI lists it as a required parameter, so generated clients cannot omit
  it; a header is easy to forget and then needs a default, which is ambiguous
  for a user in several workspaces.
- The BFF and any proxy forward it untouched.

`kernel::tenancy::Workspace<S>` is the extractor:

1. Validate the token and require scope `S` (exactly `Scoped<S>`: 401/403).
2. Read the `workspace_id` path parameter; not a UUID → 404.
3. A client token must carry the same `workspace_id` claim (#433) → else 404.
4. Ask the `Membership` port (a kernel trait, implemented by this module and
   registered in `AppState` by the server, ADR 0002/0008): for a user, are
   they a member of this live workspace and with which role; for a client,
   is the workspace live. No → 404.
5. Yield `{ principal, id, access: Access { workspace, user, role } }`.

Without a registered port (`NoMembership`), every workspace request is 404.

Users are created on their first sign-in: user-level endpoints (list my
workspaces, create a workspace, accept an invitation) upsert the `users` row
from the token (`sub`, `name`, `email`) and record a `signed_in` security
event the first time.

## Endpoints

| Method and path | Scope | Who |
| --- | --- | --- |
| `GET /api/v1/workspaces` | `workspaces:read` | user: my workspaces with my role |
| `POST /api/v1/workspaces` | `workspaces:write` | user; signup policy for a first workspace |
| `POST /api/v1/workspaces/invitations/accept` | `workspaces:write` | user whose verified email matches |
| `GET /api/v1/w/{workspace_id}` | `workspaces:read` | member or bound client |
| `PATCH /api/v1/w/{workspace_id}` | `workspaces:admin` | owner, admin |
| `DELETE /api/v1/w/{workspace_id}` | `workspaces:admin` | owner |
| `GET /api/v1/w/{workspace_id}/members` | `workspaces:read` | member or bound client |
| `PATCH /api/v1/w/{workspace_id}/members/{user_id}` | `workspaces:admin` | owner; admin for non-owners |
| `DELETE /api/v1/w/{workspace_id}/members/{user_id}` | `workspaces:admin` | owner; admin for non-owners |
| `DELETE /api/v1/w/{workspace_id}/membership` | `workspaces:write` | the member themself (leave) |
| `GET /api/v1/w/{workspace_id}/invitations` | `workspaces:admin` | owner, admin |
| `POST /api/v1/w/{workspace_id}/invitations` | `workspaces:admin` | owner, admin |
| `GET /api/v1/w/{workspace_id}/security-events` | `workspaces:admin` | owner, admin |

Role-gated actions need a user holding the role; client tokens get 403
(clients are for data integrations, not for managing people). Role rules:
only an owner may grant or take away `owner` or act on an owner; a workspace
always keeps at least one owner (`last_owner` → 409); admins manage members
and admins. Every operation's OpenAPI `operation_id` is its catalogue name,
`workspaces.<verb>[_<noun>]` (`workspaces.list`, `workspaces.create`,
`workspaces.get`, `workspaces.update`, `workspaces.delete`,
`workspaces.list_members`, `workspaces.change_role`,
`workspaces.remove_member`, `workspaces.leave`, `workspaces.invite_member`,
`workspaces.list_invitations`, `workspaces.accept_invitation`,
`workspaces.list_security_events`), since the OpenAPI document is the
operation catalogue (ADR 0013). Handlers stay thin wrappers around a plain
`execute` function with typed input and output.

## Signup policy

`CAMPBOOKS_SIGNUP__POLICY` applies only when a user with no membership
creates a workspace. A user who is already a member somewhere may create
more. The instance's first user (earliest `users` row) may always create
one, so a fresh instance can be bootstrapped under any policy.

| Policy | A user with no workspace… |
| --- | --- |
| `open` (default) | creates one |
| `invite_only` | is refused (`signup_invite_only`); joins by invitation |
| `approval` | is refused (`signup_requires_approval`); a `signup_requested` security event is recorded for the future approval tool |
| `code` | must send a `signup_code` matching one of `CAMPBOOKS_SIGNUP__CODES` (comma-separated, compared in constant time; `signup_code_invalid`). The server refuses to start with `code` and no codes. |

## Invitations

An owner or admin invites an email address as `admin` or `member` (never
`owner`). The token is `<workspace id>.<43 base64url chars of 32 random
bytes>`; only its SHA-256 is stored, it expires after 7 days and works once.
Re-inviting an address replaces its pending invitation; inviting a member's
address is a 409. The token is never returned by the API; it will be emailed
by `notifications`. Until then, with `CAMPBOOKS_ENV=development` the server
logs the accept link (`{CAMPBOOKS_PUBLIC_URL}/invitations/accept?token=…`);
in production it never logs it.

Accepting needs the token, a user token whose email is verified and equal
(case-insensitive) to the invited address: unknown token → 404, used →
409 `invitation_used`, expired → 409 `invitation_expired`, other email →
403 `invitation_email_mismatch`, already a member → 409 `already_a_member`.

## Deletion

An owner deletes a workspace: `deleted_at` and `purge_after` (now + 30 days)
are set, `WorkspaceDeleting` is published, and a `workspaces.purge_workspace`
job is enqueued for `purge_after` in the same transaction (unique per
workspace). From then on the workspace answers 404 to everyone. The job runs
with the workspace set and hard-deletes the workspace row; members,
invitations and its security events go with it (foreign keys cascade).
Other modules purge their own data when they receive `WorkspaceDeleting`.

## Events

All `workspaces.<event>`, version 1, belonging to their workspace, published
with `state.events.publish(&mut tx, …)` in the change's transaction:

| Event | Payload | Published by |
| --- | --- | --- |
| `workspace_created` | workspace, owner, kind | create |
| `member_joined` | workspace, user, role | create (the owner), accept |
| `member_left` | workspace, user, removed_by (none when leaving) | remove, leave |
| `member_role_changed` | workspace, user, from, to, changed_by | change role |
| `workspace_deleting` | workspace, purge_after, deleted_by | delete |

## Security events

Recorded in the same transaction as the change: `signed_in` (first sign-in),
`workspace_created`, `role_changed`, `invitation_created`,
`invitation_accepted`, `member_removed`, `member_left`, `workspace_deleted`,
`signup_requested`. Later sign-ins are recorded by the BFF when its code flow
completes (#16).

## Risks / Trade-offs

- The runtime role switch is per connection; a query that resets the role
  would escape it. No code runs `RESET ROLE`, and the policies are the second
  line behind the queries' own `workspace_id` filters, not the only one.
- `users` has no row-level security: it is instance-wide by nature (a user
  belongs to many workspaces). Only this module reads it.
- Policies cost a predicate per query; they compare an indexed column with a
  constant setting, so plans stay index scans.
