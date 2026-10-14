# 0015 — Instance administration: admin profiles and a separate admin API

**Status:** Accepted · 2026-10-13

## Context
An instance needs people who run it: approve signups under the `approval`
policy (today the request is recorded and nobody can act on it), hand out
signup codes, look after users and workspaces, retry failed jobs and check
the system's health. Nothing in the code knows about such a person yet.

Facts that shape the design:

- **Scopes are granted per client, not per user.** The web BFF asks for every
  API scope and the issuer grants any catalogued scope, so every signed-in
  web user's token carries all of them. A scope alone cannot mark someone as
  an administrator.
- **Row-level security hides every other workspace.** An administrator's
  questions ("which workspaces have a failing connection?") cross workspaces
  by nature.
- **The OpenAPI document is the operation catalogue (0013).** Whatever is in
  it can be invoked by the assistant, by automation and later by MCP.
- **Content is private.** Campbooks holds people's mail and paperwork.
  Running an instance must not mean being able to read them.

Owner decisions (2026-10-13): an administrator is a **user with an admin
profile**; **no endpoint ever touches admin profiles**, checked in CI;
profiles are created from the **server CLI**; permissions are **granular,
with `super` implying all of them** (the dashboard is for `super` only at
first); the dashboard is a **section of the existing client** reading a
**separate admin API** with its own permissions.

## Decision

### Admin profiles live in their own module, behind the CLI only
- A new module **`admin`** owns schema `admin`: `admin.profiles` (identity
  account id — the token `sub` — the permission set, who/when) and
  `admin.actions` (the audit trail, below).
- The module **registers no routes** in either API. Profiles are created,
  changed, listed and revoked only by `campbooks-server admin
  grant|revoke|list`, which runs on the **migration login**. The runtime role
  (`campbooks_app`) gets `SELECT` on `admin.profiles` and `INSERT` on
  `admin.actions`, nothing else, so no request handler can write a profile
  even by mistake. Every CLI change is printed and recorded in
  `admin.actions`.
- The rest of the system reads profiles only through a kernel port,
  `kernel::admin::AdminProfiles` (`permissions_of(account) -> PermissionSet`),
  implemented by the `admin` module and wired by the server, so no other
  module imports `admin`.

### Permissions
`kernel::admin::Permission` is a closed enum with stable wire names:
`super`, `system.read`, `signups.review`, `users.read`, `users.suspend`,
`workspaces.read`, `jobs.read`, `jobs.retry` (more are added with the slice
that needs them). `super` implies every permission. A permission is not an
OAuth scope and never appears in a token.

### A separate admin API, outside the operation catalogue
- A module may contribute an **`admin_router`** (new optional field on
  `kernel::Module`) next to its normal router. The server mounts all of them
  under **`/admin/v1/<module>/…`** and generates a **second document,
  `openapi/admin.json`**. Admin operations are never merged into
  `openapi/openapi.json`, so `kernel::operations::invoke`, the assistant,
  automation and MCP cannot reach them.
- Every admin handler takes **`kernel::admin::Admin<P>`**, which requires:
  1. a bearer token for a **user** (never a client-credentials client);
  2. the scope **`admin:access`**, which the issuer grants **only to
     first-party clients** (the web BFF and the native app), never to
     workspace API clients or dynamically registered ones, so a third-party
     app a super admin signs in to cannot use the admin API;
  3. **recent authentication**: the token's `auth_time` is at most one hour
     old (twelve hours until the identity audit, campbooks-cloud#117), or
     the call is refused with code `admin_reauth_required` and the client
     sends the user through sign-in again;
  4. an admin profile holding `P` (or `super`).
  Any failure of (1), (2) or (4) answers **404**, the same as a route that
  doesn't exist, so the API does not reveal who is an administrator. (3)
  applies only after (4) passes.
- The web reaches it through the BFF at **`/bff/admin/*`**, forwarded in
  process exactly like `/bff/api/*`. Native clients call it with their token.
- **Every admin request is audited.** A middleware on the admin router
  records actor, `operationId`, path parameters, result status and time in
  `admin.actions` through the `kernel::admin::AdminAudit` port, reads
  included.

### Seeing across workspaces, narrowly
- `kernel::db::begin_admin(pool, &AdminGrant)` opens a transaction with the
  setting `campbooks.admin_reads = on`. `AdminGrant` can only be built by the
  `Admin<P>` extractor.
- A module that offers admin views adds **`FOR SELECT` policies keyed on
  that setting, on metadata tables only** (users, workspaces, members,
  connections, streams, security events). **Content tables never get one**
  (messages, bodies, attachments, documents and anything later declared as
  content). Admin mutations (approve a signup, suspend a user, retry a job)
  are ordinary module functions with their own narrow policies.
- Admin views return metadata: counts, states, timestamps, provider names,
  addresses the user signed in with. No message, subject, body or document
  content, and no impersonation.

### What CI checks
`cargo xtask arch` gains:
- the `admin` module's normal router and admin router are empty, and no
  operation in either document is owned by `admin`;
- no Rust source outside `crates/modules/admin` and the server's CLI
  references schema `admin`, and only the kernel references the
  `campbooks.admin_reads` setting (a source scan that reuses the migration
  SQL scanner);
- every admin operation: `operationId` `<module>.<verb>[_<noun>]` owned by
  the module that serves it, path under `/admin/v1/<module>/`, security
  scheme `admin` (scope `admin:access`) and an `x-campbooks-admin.permission`
  that names a known permission; `openapi/admin.json` is up to date
  (`cargo xtask openapi --check` covers both documents).

A server test pins the set of tables that carry an admin-read policy against
an allowlist and fails if a content table is in it.

### The dashboard
An **Admin** section of `clients/app` at `/admin`, outside any workspace,
using `lib/ui` and the four locales. The shell lives in `app/admin/`; each
client module contributes its panels through its `index.ts`, mirroring the
server modules. Admin types are generated from `openapi/admin.json` into
their own file. The entry in the account menu appears only when
`GET /admin/v1/system/overview` succeeds; for everyone else it 404s and
nothing is shown.

## Consequences
- Each module keeps owning its tables and its queries; the admin view of
  connectors is written next to connectors' own code and reuses its domain
  functions.
- Becoming an administrator requires shell access to the server, which is
  also what a self-hoster already has. The cloud's first super admin is
  granted by the operator after the first sign-in.
- One more document to keep generated, one more router field, one more
  extractor. In exchange, admin power is unreachable from the assistant,
  third-party apps and tokens older than an hour, and every use leaves a
  trace.
- Revoking a profile takes effect on the next request (profiles are read per
  request, not cached in tokens).

## Alternatives considered
1. **A central `campbooks-admin` crate that queries every schema.** One
   place for all admin code and no new router field, but it needs SQL across
   every module's schema (which 0002 and the migration checks forbid) and
   would rot as modules change their tables. Rejected.
2. **Admin operations in the main catalogue behind `admin:*` scopes.** No
   second document, but every web token already carries every scope, and the
   assistant and automation could invoke admin operations by id. Rejected.
3. **A role column on the user, managed from the UI** (the old Rails app's
   `admin` flag and `/admin` controllers). Simple, but administrator status
   becomes something an endpoint can grant, which the owner ruled out.
   Rejected; this ADR deliberately does not copy it.
4. **A generic database admin UI** (Adminer, pgweb) behind the auth gate.
   Nothing to build, but it bypasses row-level security, shows content and
   leaves no audit trail. Rejected.
5. **A separate admin web app and origin.** Could be fenced to an internal
   network, but doubles the client, BFF and deployment. The owner chose the
   in-app section.
