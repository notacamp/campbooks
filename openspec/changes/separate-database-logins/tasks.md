# Tasks

## 1. Kernel

- [x] 1.1 `RoleReport` / `RoleVerdict`, `inspect_role`, `connect_options` (password file), `connect_runtime` (SET ROLE only for a privileged login); verify with unit and `#[sqlx::test]` tests
- [x] 1.2 Config: `CAMPBOOKS_DATABASE_MIGRATION_URL`, `CAMPBOOKS_DATABASE_PASSWORD_FILE`, `CAMPBOOKS_DATABASE_ALLOW_RLS_BYPASS`; verify parsing and blank handling

## 2. Server

- [x] 2.1 Migrations on the migration login; pool on the runtime login; refuse or warn at startup (`check_row_security`); verify against the dev Postgres for split, legacy, empty-role, escape hatch and split-with-superuser

## 3. Deploy

- [x] 3.1 `db-init` and `deploy/postgres/init.sh`, `db-secrets` volume, server wiring; `docker compose config -q`; init SQL applied twice against Postgres
- [x] 3.2 `deploy/README.md` "Database logins", `.env.example`, CHANGELOG
