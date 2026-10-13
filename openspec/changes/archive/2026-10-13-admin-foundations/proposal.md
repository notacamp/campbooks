# Proposal

## Why

Nobody can run a Campbooks instance from inside it. The `approval` signup
policy records requests that no one can approve, signup codes live in an
environment variable, failed jobs can only be found in the database, and
there is no notion of an administrator at all. This change lays the
foundation every admin feature builds on: who an administrator is, how an
admin request is authorised and audited, how it may see across workspaces,
and the dashboard shell — with a first panel, **System**.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/96
(epic https://github.com/notacamp/campbooks-cloud/issues/95). Design:
ADR 0015.

## What Changes

- **New module `admin`** (schema `admin`): `admin.profiles` and
  `admin.actions`. It registers **no routes**. It implements the kernel ports
  `AdminProfiles` and `AdminAudit`. New module ⇒ ADR 0015; no
  `architecture.toml` entry (nobody depends on it).
- **Server CLI** `campbooks-server admin grant|revoke|list`, on the migration
  login, recorded in `admin.actions`. The only way to create, change or remove
  a profile.
- **Kernel**: `admin::{Permission, PermissionSet, Admin<P>, AdminGrant,
  AdminProfiles, AdminAudit}`, `db::begin_admin`, and an optional
  `admin_router` on `Module`.
- **Identity**: scope `admin:access`, issued only to first-party clients.
- **Server**: admin routers mounted at `/admin/v1/<module>/…` with the audit
  middleware; `/bff/admin/*` forwarding; a second generated document
  `openapi/admin.json`.
- **xtask**: the CI rules of ADR 0015 (empty admin module, source scan for
  schema `admin` and `campbooks.admin_reads`, admin operation checks, both
  documents checked).
- **system**: `GET /admin/v1/system/overview` (`system.read`): version and
  commit, latest migration, uptime, effective configuration without secrets
  (signup policy, sign-in providers configured, blob store kind, mail
  providers configured), AI roles (configured, model, reachable), and the
  database's health.
- **Client**: Admin section at `/admin` (shell, nav, System panel), the
  account-menu entry shown only to administrators, admin API types from
  `openapi/admin.json`, the re-authentication prompt, four locales.

## Non-goals

- Signups, users & workspaces and jobs panels (S1–S3, #97–#99).
- Any endpoint that reads or changes admin profiles, now or later.
- Permissions narrower than `super` in the UI (the model supports them; the
  dashboard is shown to `super` only).
- Reading users' content, impersonation, break-glass access.
- Cloud-only panels (billing, plans).
