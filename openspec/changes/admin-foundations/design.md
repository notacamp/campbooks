# Design

ADR 0015 is the decision record; this file covers the details of slice S0.

## Data ownership and events

- `admin` module, schema `admin`:
  - `admin.profiles (account_id text primary key, permissions text[] not null
    check (permissions <> '{}'), note text, granted_at timestamptz, updated_at
    timestamptz)`. `account_id` is the identity account id (the token `sub`),
    referenced by id only (no cross-schema foreign key). Each permission is
    one of the `kernel::admin::Permission` wire names (checked in Rust; the
    CLI refuses unknown names).
  - `admin.actions (id uuid v7, at, actor text, via text check in
    ('api','cli'), operation text, target jsonb, status smallint)`; append-only
    — the runtime role has `INSERT` only, no `UPDATE`/`DELETE`.
  - Grants: `campbooks_app` gets `USAGE` on schema `admin`, `SELECT` on
    `profiles`, `INSERT` on `actions`. The migration login owns both tables.
  - No RLS on `admin` (not workspace-owned; same as `kernel`, `identity`).
- No domain events in S0. (A future "profile granted" event would be
  published by the CLI only.)

## Authorisation (`kernel::admin::Admin<P>`)

Order: bearer token verified → principal is a `User` → scope `admin:access`
→ `AdminProfiles::permissions_of(sub)` contains `P` or `super` → otherwise
**404** (`kernel::Error::NotFound`). Then `auth_time` ≤ 12 h old, otherwise
**401** with code `admin_reauth_required` (`WWW-Authenticate` error
`insufficient_user_authentication`, `max_age=43200`, RFC 9470). The BFF on a
401 with that code answers the client with the same problem; the client
redirects to `/bff/login?max_age=0&return_to=/admin…` (the BFF passes
`max_age`/`prompt=login` to the issuer).

`P` is a type implementing `AdminPermission` (`const PERMISSION:
Permission`), mirroring `ScopeName`. The extractor yields `AdminGrant {
account, permissions }`; `AdminGrant`'s constructor is `pub(crate)` in the
kernel.

Scope: `admin:access` is declared by the `admin` module (`Module.scopes`). The
identity issuer's client registry gains `first_party: bool`; `allowed_scope`
drops `admin:access` for a client that is not first-party. The BFF's default
scope list includes it.

## Admin router and documents

`kernel::Module` gains `admin_router: Option<OpenApiRouter<AppState>>`. The
server nests each under `/admin/v1/<module>` (the module's paths are relative),
adds the audit layer and the `admin` security scheme (bearer, scope
`admin:access`), and serves the result at `/admin/v1/*` — **not** through
`operations::compose`, so `invoke` and the idempotency layer of the catalogue
don't see it (admin mutations are idempotent by design or carry their own
`Idempotency-Key` handling in their slice). `campbooks_server::admin_openapi()`
builds the document; `cargo xtask openapi [--check]` writes/compares
`openapi/admin.json` alongside `openapi.json`; `GET /admin/v1/openapi.json` is
**not** served (the document is a build artefact).

Each admin operation carries `x-campbooks-admin: { permission, mutates }`.

The audit layer runs after the extractor has resolved (it reads the
`AdminGrant` from request extensions placed by the extractor) and records
every request that reached a handler; requests that 404 before the profile
check are not recorded (no actor).

## Cross-workspace reads

`kernel::db::begin_admin(&PgPool, &AdminGrant)` → transaction with
`set_config('campbooks.admin_reads','on',true)`. S0 uses it nowhere (System
reads no workspace data); it lands with the CI rule so S1–S3 build on it. The
pinned allowlist test starts empty.

## System overview

`system` gains `features/admin_overview` with `GET /admin/v1/system/overview`
(operationId `system.admin_overview`, permission `system.read`). Sources:
`kernel::config` (redacted view — a new `Config::summary()` that returns only
non-secret, enumerated facts), `_sqlx_migrations` (latest version + count),
process start time, `campbooks-ai` gateway (`roles()` + a reachability probe
per configured role: `GET {base}/models` with a 3 s timeout, cached 60 s in
memory). No secret, URL credential or API key ever appears; base URLs are
reduced to scheme + host.

## Client

- `clients/app/src/app/admin/`: `AdminLayout` (sidebar: System; later
  Signups, People, Workspaces, Jobs), route `/admin` outside the workspace
  layout, redirect `/admin` → `/admin/system`.
- `lib/api/admin.gen.ts` from `openapi/admin.json`; `adminClient` with base
  `/bff/admin` on web, the native transport on Tauri. `pnpm api:check` covers
  it.
- `modules/system` contributes `SystemAdminPanel` via `index.ts`.
- Account menu: "Admin" item when `useAdminAccess()` (query on
  `system.admin_overview`, `retry: false`, 404 ⇒ hidden).
- `admin_reauth_required` → full-page notice with a "Sign in again" button.
- Every string in en, pt-PT, es-ES, fr.

## Alternatives considered

See ADR 0015 (central admin crate; scopes in the main catalogue; role column
managed from the UI — the old Rails app's shape, deliberately not copied;
generic DB admin UI; separate admin app).

## Risks and trade-offs

- The identity client registry is also being changed by S4 (#81, workspace
  clients and DCR). Whoever merges second adds `first_party: false` to the new
  clients; the rule "non-first-party never gets `admin:access`" is tested on
  the issuer, so it holds for clients added later.
- A super admin who loses shell access to the server cannot get admin back
  from the UI — by design.
