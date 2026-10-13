# Design

## Context

#442 made every pooled connection `SET ROLE campbooks_app`, so policies apply
even when the login is a superuser. The role stays reversible, though.

## Data ownership and events

No new tables, events or scopes. The kernel owns the role handling
(`kernel::db`); the migrations and their grants are unchanged. The one new
cluster object is the `campbooks_runtime` login, created by deployment tooling
(compose `db-init`, or the administrator's SQL), never by a migration.

## Decisions

### A login that is itself unprivileged
The runtime login is a member of `campbooks_app` and inherits its grants, so
it needs no `SET ROLE`. `connect_runtime` probes the login once and issues
`SET ROLE` only if the login can bypass row-level security (the legacy path).
The verdict logic (`RoleReport::verdict`) is a pure function of what Postgres
reports (`pg_roles` + membership + ownership of policy tables), unit tested.

### Refuse, not warn
Isolation is the product's core promise; a log line nobody reads is how it
quietly stops being true. Refusing is safe because the only previously working
setup that now fails is "superuser URL and an empty `CAMPBOOKS_DATABASE_ROLE`",
and the escape hatch `CAMPBOOKS_DATABASE_ALLOW_RLS_BYPASS` covers it. The
default single-URL setup (superuser + `SET ROLE`) only warns, so upgrades do
not break; asking for the split (setting a migration URL) opts into the strict
check.

### Password from a file
`CAMPBOOKS_DATABASE_PASSWORD_FILE` follows the volume pattern of `idp-init`:
the secret never enters `.env` or the process environment. The file is mode 0440, group 10001 (the
server's supplementary group, as `server-init`'s files).

## Alternatives considered

1. **Keep `SET ROLE` and harden it** (e.g. `SET SESSION AUTHORIZATION`, or
   `SET ROLE` with a role the login cannot leave). Postgres has no
   non-resettable role switch for a session, except `SET SESSION
   AUTHORIZATION` for superusers, which is also resettable. Rejected: no new
   machinery, but does not fix the problem.
2. **Create the runtime login from the server** at startup using the owner
   URL. One URL less for the operator, but the server process would keep
   superuser-class credentials in its environment, which is what we remove.
   Rejected.
3. **Chosen: a second login, provisioned by deployment.** One new init
   container and a volume for self-hosters (no manual steps), one SQL snippet
   for managed Postgres. New moving part: `db-init`; the source of truth for
   the password is the `db-secrets` volume.

Does not copy a pattern from the old Rails app on `main`, which used a single
database user.

## Risks and trade-offs

- A managed database whose administrator cannot create roles cannot split; the
  single-URL path keeps working with a warning.
- A restore of Postgres into a fresh cluster loses the login's password
  (cluster-wide objects); `db-init` re-applies it from the volume on the next
  `up`.
