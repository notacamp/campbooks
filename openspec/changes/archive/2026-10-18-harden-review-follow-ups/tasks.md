## 1. Keys and secrets

- [x] 1.1 Idempotency keys per person and app (`user:<len>:<subject>|azp:<client>`)
- [x] 1.2 `cargo xtask arch`: secret-looking answer fields need `secret_once` or a listed reason
- [x] 1.3 `identity.token` declares `secret_once`; answers below 400 withheld, a redirect's `Location` not kept

## 2. Database roles and coverage

- [x] 2.1 Startup refuses a role holding `campbooks_definer`; a migration refuses `campbooks_app` holding it
- [x] 2.2 The migration login holds `campbooks_definer` with `USAGE` (granted on PostgreSQL 16+, else named)
- [x] 2.3 The coverage test reads views and materialized views and refuses open permissive policies

## 3. Startup and migrations

- [x] 3.1 Refuse PostgreSQL older than 15
- [x] 3.2 Migration lock waits capped at a minute after the migration lock
- [x] 3.3 A migration rebuilds missing or invalid job key indexes

## 4. BFF and docs

- [x] 4.1 No touch with a deadline not after the use (clock skew)
- [x] 4.2 Self-hosting docs: what the separation protects, PostgreSQL version, definer grants, lock waits

## 5. Tests

- [x] 5.1 Unit and integration tests for each item above
