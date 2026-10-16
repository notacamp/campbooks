# Tasks

## 1. Kernel

- [x] 1.1 `tenancy`: `WorkspaceId`, `UserId`, `Role`, `Access`, the `Membership` port (`NoMembership` default in `AppState`) and the `Workspace<S>` extractor; verify 401/403 before membership, 404 for bad ids, foreign client claims and non-members
- [x] 1.2 `db`: `BeginFor::begin_for`, the shared `campbooks.workspace_id` setter (also used by `JobContext::tx`), the runtime-role pool options and the bypass check; `testing::app_role_pool`
- [x] 1.3 `Principal` gains `email`, `email_verified`, `name`; `Error::Denied(code)`; config `CAMPBOOKS_ENV`, `CAMPBOOKS_SIGNUP__*`, `CAMPBOOKS_DATABASE_ROLE` with startup checks
- [x] 1.4 Kernel migration: `campbooks_app` role (idempotent, safe under concurrent test databases) + grants on `kernel`; system migration grants `system.instance`

## 2. Module `workspaces`

- [x] 2.1 Crate, scopes, migration (tables, indexes, RLS enabled + forced, policies, grants), registration in the server
- [x] 2.2 Users on first sign-in (`signed_in` security event); `Membership` implementation
- [x] 2.3 List my workspaces, create (signup policy, `WorkspaceCreated` + `MemberJoined`)
- [x] 2.4 Get, update (name, accent), delete (soft, `WorkspaceDeleting`, delayed `workspaces.purge_workspace` job)
- [x] 2.5 Members: list, change role, remove, leave (last-owner protection, `MemberRoleChanged`, `MemberLeft`)
- [x] 2.6 Invitations: create (dev-only link), list pending, accept (expiry, single use, verified email, `MemberJoined`)
- [x] 2.7 Security log endpoint
- [x] 2.8 Every operation has an `operation_id` of the form `workspaces.<verb>[_<noun>]` (ADR 0013: the OpenAPI document is the operation catalogue)

## 3. Tests

- [x] 3.1 RLS blocks cross-workspace reads at the SQL level; every workspace-owned table has RLS forced and grants for `campbooks_app`
- [x] 3.2 Non-member → 404; client token bound to its workspace; anonymous → 401
- [x] 3.3 Last-owner protection; role rules
- [x] 3.4 Invitation expiry, single use, wrong email, unverified email
- [x] 3.5 Signup policies (open, invite_only, approval, code, first-user bootstrap)
- [x] 3.6 Events published transactionally; soft delete → `WorkspaceDeleting` → purge job hard-deletes

## 4. Follow-ups (not in this change)

- [x] 4.1 Add the `x-campbooks` operation extension (mutates, undo, automation, assistant, risk, records) to every `workspaces.*` operation once #50 lands its schema and xtask check (ADR 0013); operation ids already follow `workspaces.<verb>[_<noun>]` — shipped: every `workspaces.*` operation carries `x-campbooks` (`operation-catalogue`)
- [x] 4.2 Email invitations through `notifications`; revoke an invitation; restore a workspace during its grace period — moved to campbooks-cloud#193
- [x] 4.3 Instance administration: approve `approval` signups, single-use signup codes (hosted beta codes) — shipped: admin approve/mint/revoke signup codes (`admin-signups`)
- [x] 4.4 Sign-in security events from the BFF code flow (#16) — moved to campbooks-cloud#194
- [x] 4.5 Register per-workspace API clients with the IdP (ADR 0004) — shipped: `register_client`/`rotate_client_secret` (`identity-clients-consent`, cloud#81)
- [x] 4.6 Web client: workspace switcher, members and invitations screens — shipped: workspace switcher (`first-run-ui`), `MembersPage`, `AcceptInvitation`

## 5. Wrap-up

- [x] 5.1 OpenAPI regenerated; client types regenerated
- [x] 5.2 `docs/architecture.md` (request flow, runtime role), `deploy/README.md`, `deploy/.env.example`, CHANGELOG `[Unreleased]`
- [x] 5.3 Gates green: fmt, clippy, tests, `cargo xtask arch`, `cargo xtask openapi --check`, sqlx prepare check, `cargo deny`, `cargo machete`, `openspec validate --all --strict`
- [x] 5.4 UI: none
