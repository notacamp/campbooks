# Tasks

## 1. Callers and storage

- [x] 1.1 List every reader and writer of `bff.sessions`, `bff.login_transactions` and `bff.retired_tokens` (handlers, lookup, refresh, logout, back-channel, the two account handlers, the prune, admin, tests) with its key and pool; record them in `design.md`
- [x] 1.2 Check what each column stores at rest; seal the sign-in's nonce, the one plain-text value

## 2. The boundary

- [x] 2.1 Migration: delete in-flight sign-ins, the nonce as `bytea`, revoke every privilege of the runtime role (and `PUBLIC`) on the three tables, row-level security enabled with no policy
- [x] 2.2 Migration: fifteen `SECURITY DEFINER` functions (pinned path, qualified names, validated input), executable by the runtime role only
- [x] 2.3 `bff::store` calls only the functions; sessions are named by the presented token's hash (`Session::presented`); the replaced queries' offline cache entries removed
- [x] 2.4 The prunes: bounded batches, the database's clock, the condition re-checked on the deleted row

## 3. Follow-ups (campbooks-cloud#213)

- [x] 3.1 `cargo xtask arch`: a pinned path with another entry, `pg_temp` not last, or ending the file is refused; unit tests
- [x] 3.2 `CAMPBOOKS_CHANGES__OPENS_WINDOW_SECONDS` bounded to a day at startup; unit test
- [x] 3.3 Kernel migration: `kernel.prune_rate_limits` re-checks the window it deletes

## 4. Tests

- [x] 4.1 The runtime role cannot select, update, delete, insert, truncate or lock the three tables
- [x] 4.2 Sign-in, rotation and reissue, refresh, sign-out, sign-out everywhere, suspension and the prune work through the functions as the runtime role
- [x] 4.3 A function never answers or ends another account's session
- [x] 4.4 Nothing secret is stored in plain text (hashes and sealed values, bound to their row)
- [x] 4.5 Coverage: every `kernel` and `bff` table has forced row-level security, no runtime privilege, or a reason; every definer function pins its path and is not `PUBLIC`; the access matrix lists the three tables as unreadable

## 5. Wrap-up

- [x] 5.1 Rollback SQL checked against a database migrated to the base (identical schema dump of `bff` and `kernel`, grants and functions included, same `_sqlx_migrations`), and re-upgrading applies cleanly
- [x] 5.2 OpenAPI unchanged, client types unchanged, events unchanged
- [x] 5.3 CHANGELOG updated
- [x] 5.4 Gates green (fmt, clippy, the whole workspace's tests, xtask arch / migrations / openapi / events / i18n, cargo deny)
