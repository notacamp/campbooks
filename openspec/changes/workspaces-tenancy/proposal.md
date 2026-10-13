# Proposal

## Why

Every domain module (connectors, conversations, parties, …) keeps data that
belongs to one workspace and must never be seen from another. Before any of
them can ship, Campbooks needs the tenancy foundation: who a user is (keyed
by the identity provider's subject), which workspaces they belong to and with
what role, how people are invited, who may create a workspace on an instance,
and a log of security-relevant changes. ADR 0011 also asks for row-level
security as a second line of defence behind the queries' own filters, with
the workspace set on every request's and job's transaction.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/18.

## What Changes

- **New module `workspaces`** (`crates/modules/workspaces`, schema
  `workspaces`): users, workspaces, members with a role (`owner`, `admin`,
  `member`), invitations, security events. A user can belong to many
  workspaces.
- **Endpoints** (scopes `workspaces:read`, `workspaces:write`,
  `workspaces:admin`): list my workspaces, create a workspace (the creator
  becomes its owner), read and update a workspace (name, accent), list
  members, change a member's role, remove a member, leave, invite by email,
  list pending invitations, accept an invitation, delete a workspace (soft
  delete, hard delete after a grace period), read the security log.
- **Events** published in the transaction of the change: `WorkspaceCreated`,
  `MemberJoined`, `MemberLeft`, `MemberRoleChanged`, `WorkspaceDeleting`.
- **Signup policy** for creating a first workspace,
  `CAMPBOOKS_SIGNUP__POLICY=open|invite_only|approval|code` (default `open`),
  with `CAMPBOOKS_SIGNUP__CODES` for `code`.
- **Kernel: tenancy seam.** `WorkspaceId`, `UserId`, `Role`, a `Membership`
  port the server registers in `AppState`, and the `Workspace<S>` extractor:
  token + scope (as `Scoped<S>`), then the workspace from the path
  `/api/v1/w/{workspace_id}/…`, then membership (users) or the token's
  workspace claim (clients); anything else answers 404.
- **Kernel: row-level security plumbing.** `PgPool::begin_for(workspace)`
  (via the `BeginFor` trait) opens a transaction with
  `campbooks.workspace_id` set, the same setting jobs use. The server's pool
  switches every connection to a runtime role (`campbooks_app` by default,
  `CAMPBOOKS_DATABASE_ROLE`) that is subject to row-level security;
  migrations keep running as the connecting user.
- **Kernel: identity claims.** `Principal` carries the user's `email`,
  `email_verified` and `name` from the token.
- **Kernel: `Error::Denied(code)`** — a 403 with a machine-readable reason
  (`signup_invite_only`, `last_owner`, …).
- **Kernel config:** `CAMPBOOKS_ENV` (`production` default, `development`)
  and the signup settings.

## Capabilities

### New Capabilities

- `workspaces`: users, workspaces, membership and roles, invitations, signup
  policy, the security log, workspace deletion, workspace resolution on
  requests, and row-level security on workspace-owned tables.

### Modified Capabilities

None (`events-and-jobs` is unchanged: jobs already set
`campbooks.workspace_id`; this change adds the policies that read it).

## Impact

- New crate `campbooks-workspaces`; registered in `campbooks-server`. No
  module depends on it and it depends only on the kernel. No
  `architecture.toml` entry.
- Kernel API additions (`tenancy`, `db`, `Principal` fields,
  `Error::Denied`, config). `AppState` gains `membership`.
- Data: new schema `workspaces`; a kernel migration creates the
  `campbooks_app` role and grants it the kernel tables; a `system` migration
  grants it `system.instance`.
- Dependencies: the module uses the workspace's existing `aws-lc-rs`
  (randomness, SHA-256, constant-time compare) and `base64`. No new crate.
- Self-hosters: new optional settings `CAMPBOOKS_SIGNUP__POLICY`,
  `CAMPBOOKS_SIGNUP__CODES`, `CAMPBOOKS_ENV`, `CAMPBOOKS_DATABASE_ROLE`.
  Nothing to do on upgrade: migrations create the role (the compose
  database user can).
- OpenAPI: new `workspaces` operations; scope catalogue gains three scopes
  (the IdP bootstrap mirrors them from `campbooks-server scopes`).

## Non-goals

- Emailing invitations. The link is logged only when `CAMPBOOKS_ENV=development`;
  delivery comes with `notifications`.
- Instance administration: approving `approval` signups, minting single-use
  signup codes, an instance admin role. `approval` records the request and
  refuses; `code` accepts reusable codes from configuration.
- Restoring a workspace during its grace period, ownership transfer as its
  own action (an owner can promote another member to owner), revoking an
  invitation.
- Registering per-workspace API clients with the IdP (ADR 0004).
- The `x-campbooks` operation extension (ADR 0013, issue #50). Operations
  already carry catalogue ids (`workspaces.list`, `workspaces.invite_member`,
  …); the extension is added when #50 defines it.
- Client UI. The web client gets workspace screens in a later change.
