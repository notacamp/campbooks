# Proposal

## Why

Row-level security (ADR 0011) only protects a connection whose role is not a
superuser and has no `BYPASSRLS`. The server reaches that by connecting as the
configured login and running `SET ROLE campbooks_app` on every pooled
connection. A SQL injection (or any code path that runs attacker-influenced
SQL) can `RESET ROLE` and is back to the login's full power, with every
workspace's rows. The runtime connection should be a login that has nothing to
climb back to.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/60.

## What Changes

- **Two logins.** `CAMPBOOKS_DATABASE_URL` is the runtime login (non-superuser,
  no `BYPASSRLS`, owns nothing, member of `campbooks_app`);
  `CAMPBOOKS_DATABASE_MIGRATION_URL` (optional) is the owner login used only to
  apply migrations. `CAMPBOOKS_DATABASE_PASSWORD_FILE` supplies the runtime
  password from a file.
- **Startup check.** The server inspects the role it runs as and refuses to
  start when it is, or belongs to, a superuser / `BYPASSRLS` role, or owns
  tables with policies; with a migration login configured it also refuses a
  runtime login that could switch to a privileged role.
  `CAMPBOOKS_DATABASE_ALLOW_RLS_BYPASS=true` overrides.
- **Compatibility.** A single privileged URL keeps working through `SET ROLE`
  (only when the login is privileged; an unprivileged login is used as it is),
  with a warning recommending the split.
- **Compose.** A one-shot `db-init` creates the `campbooks_runtime` login,
  generates its password into a `db-secrets` volume and grants it
  `campbooks_app`; the server reads the password from the file. Existing
  installations are upgraded by the same `docker compose up -d`.
- **Docs.** `deploy/README.md` "Database logins" (bundled and managed
  Postgres), `.env.example`, CHANGELOG.

## Non-goals

- Per-module or per-schema database logins.
- Creating the login from the server itself (it would need the privileges this
  change removes from it at runtime).
- Rotating the runtime password automatically.
- Changing the `campbooks_app` role, the policies or any migration.
