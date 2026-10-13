# Tasks

## 1. Kernel

- [x] 1.1 `kernel::admin`: `Permission` (wire names, `super` implies all), `PermissionSet`, `AdminPermission` trait, `AdminProfiles` + `AdminAudit` ports, `AdminGrant` (crate-private constructor); unit tests
- [x] 1.2 `Admin<P>` extractor: user principal, `admin:access`, profile check → 404, `auth_time` ≤ 12 h → 401 `admin_reauth_required` (localized reason in all four catalogues); tests for each refusal
- [x] 1.3 `db::begin_admin`; `Module.admin_router: Option<…>`

## 2. Admin module

- [x] 2.1 Create `crates/modules/admin` (by hand, in the generator's shape: it has no routes, no client module and no own change); migration `admin__init` (profiles, actions, grants: SELECT profiles / INSERT actions for `campbooks_app`)
- [x] 2.2 Port implementations (`permissions_of`, `record`); scope `admin:access` in `Module.scopes`
- [x] 2.3 CLI functions `grant`/`revoke`/`list`/`log` (email → account via identity's lookup port or `--account`), recording `via = cli`; `#[sqlx::test]`s incl. runtime-role write refused

## 3. Identity

- [x] 3.1 `Client.first_party`; `allowed_scope` drops `admin:access` for non-first-party; BFF default scopes include it; issuer test

## 4. Server

- [x] 4.1 Mount admin routers at `/admin/v1/<module>`, `admin` security scheme, audit middleware; `admin_openapi()`; `campbooks-server admin grant|revoke|list` (clap) on the migration login
- [x] 4.2 BFF `/bff/admin/*` forwarding (same CSRF/origin rules as `/bff/api/*`), `max_age` pass-through on `/bff/login`
- [x] 4.3 Tests: anonymous / member / client token / no scope → 404; super → 200; stale → 401; invoke refuses admin ids; admin-read policy allowlist (empty) test

## 5. xtask

- [x] 5.1 `openapi` writes/checks `openapi/admin.json` too
- [x] 5.2 `arch`: admin module owns no operations/routes; source scan for schema `admin` and `campbooks.admin_reads`; admin operation checks (path prefix, owner, `x-campbooks-admin.permission` known, security `admin`); unit tests for each rule

## 6. System overview

- [x] 6.1 `Config::summary()` (no secrets), AI role probe (3 s timeout, 60 s cache) in `campbooks-ai`
- [x] 6.2 `system/features/admin_overview`; tests incl. redaction

## 7. Client

- [x] 7.1 `admin.gen.ts` + `adminClient`; `api:check` covers it
- [x] 7.2 `app/admin` shell + route + account-menu entry (`useAdminAccess`), re-auth notice; System panel from `modules/system`; i18n ×4; Storybook stories
- [x] 7.3 Playwright: member sees no entry; super admin sees System; recordings 375 px + desktop

## 8. Wrap-up

- [x] 8.1 Docs: `deploy/README.md` "Administrators" (granting the first super admin), `docs/architecture.md` admin section, ADR index
- [x] 8.2 OpenAPI regenerated (both), client types regenerated, CHANGELOG updated, gates green, UI verified at 375 px and desktop
