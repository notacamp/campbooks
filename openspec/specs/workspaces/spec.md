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

### Requirement: Unverified addresses are not shown as members'
The workspaces module SHALL record whether the identity provider verified a
user's address at their last sign-in. A member's address SHALL be listed
(`workspaces.list_members`, `workspaces.change_role`) only when verified, and
inviting an address SHALL treat it as a member's only when a member verified
it.

#### Scenario: A claimed address
- **WHEN** a person whose provider did not verify `ceo@example.com` creates a workspace and its members are listed
- **THEN** their entry has no address, and after a sign-in where the provider verifies it the address is listed
