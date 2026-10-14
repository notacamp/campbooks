# Proposal

## Why

An administrator can see that the instance runs (the System panel, S0) but
not who uses it: nobody can answer "who has an account, in which
workspaces, and are their mailboxes healthy?", and nobody can stop a
compromised or abusive account from signing in short of editing the
database. This is slice S2 of the admin dashboard: **People** and
**Workspaces**, metadata only, and **suspending** an account.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/98
(epic https://github.com/notacamp/campbooks-cloud/issues/95). Builds on
ADR 0015 and the admin foundation (S0).

## What Changes

Three modules each serve admin operations over their own tables; none reads
another's schema, and the client stitches the answers by id.

- **workspaces** (`users.read`, `workspaces.read`):
  `GET /admin/v1/workspaces/users` (search by name or address prefix,
  cursor paging), `GET /admin/v1/workspaces/users/{user_id}` (their
  workspaces and roles), `GET /admin/v1/workspaces/workspaces` (name prefix,
  cursor paging: member count, owners) and
  `GET /admin/v1/workspaces/workspaces/{workspace_id}` (members and roles,
  pending invitations as a count). New admin-read policies on
  `workspaces.workspaces`, `members` and `invitations`.
- **identity** (`users.read`, `users.suspend`):
  `GET /admin/v1/identity/accounts/{account_id}` (primary address, ways in,
  passkey count, live sessions, last sign-in, suspension),
  `POST …/suspend` and `POST …/unsuspend`. Suspending sets
  `identity.accounts.disabled_at`, ends every issuer session, revokes every
  refresh-token family, records `account_suspended` in
  `identity.audit_events` and publishes the new event
  `identity.account_suspended`. An account with an admin profile is refused
  (`account_is_admin`, checked through the kernel's `AdminProfiles` port).
  The provider sign-in's refusal code becomes `account_suspended` (it was
  `account_disabled`, never reachable before: nothing set `disabled_at`).
- **connectors** (`workspaces.read`):
  `GET /admin/v1/connectors/workspaces/{workspace_id}/connections`:
  provider, address, capabilities, health and each stream's phase, last sync
  and failures. New admin-read policies on `connectors.connections` and
  `connectors.streams`.
- **server**: the web BFF consumes `identity.account_suspended`
  (`bff.end_suspended_sessions`) and deletes the account's web sessions at
  once. The pinned admin-read allowlist gains the five tables above.
- **client**: Admin → **People** (list with search, a person's page with
  their account, ways in, sessions, workspaces, and Suspend / Unsuspend with
  an inline confirmation that says what happens) and Admin → **Workspaces**
  (list with search, a workspace's page with members and connection health).
  Four locales; the sign-in screen's `account_suspended` message.

No new crate. No new module, so no `architecture.toml` entry: the BFF is
the server's, which already composes the identity module.

## Non-goals

- Reading or counting anyone's content: messages, subjects, documents,
  drafts. No admin-read policy on any content table, ever.
- Impersonation, changing a person's ways in, deleting accounts or
  workspaces, changing roles from the admin section.
- Admin profiles over HTTP (ADR 0015): suspending an administrator needs the
  CLI to revoke their profile first.
- Revoking access tokens already issued (they live ten minutes; the API does
  not look up the account on every request). Recorded as a follow-up.
- A suspended badge in the People list (the list is the workspaces module's;
  suspension is identity's). The person's page shows it.
- Signups (S1) and jobs (S3).
