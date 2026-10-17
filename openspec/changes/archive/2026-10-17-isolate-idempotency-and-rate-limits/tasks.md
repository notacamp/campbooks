# Tasks

## 1. Callers

- [x] 1.1 List every reader and writer of `kernel.idempotency` and `kernel.rate_limits` (middleware, prunes, admin, tests) with its pool or transaction and what it knows; record them in `design.md`

## 2. Idempotency keys

- [x] 2.1 `db::begin_as_idempotency_caller` and the crate-private `campbooks.idempotency_caller` setting; claim, store and release run in the caller's transaction
- [x] 2.2 The prune runs in the kernel scope, one batch per transaction
- [x] 2.3 `Operation::secret_once` / `x-campbooks.secret_once`; the five operations declare it; `openapi.json` updated
- [x] 2.4 A secret's answer is kept as `withheld` (no body); a replay answers 409 `idempotency_secret_shown_once` in the four languages
- [x] 2.5 Migration: delete the five operations' cached rows, the `withheld` state and its no-body check, forced row-level security with the caller and expired-only kernel policies

## 3. Rate-limit counters

- [x] 3.1 Migration: revoke the runtime role's privileges, `kernel.take_rate_limit` and `kernel.prune_rate_limits` (definer, pinned path, validated input, the caller's scope restored), forced row-level security
- [x] 3.2 `RateLimits::hit`, `exhaust` and the prune go through the functions; the replaced queries' offline cache entries removed

## 4. Checks and tests

- [x] 4.1 Server test: every kernel table has forced row-level security or an instance-only reason
- [x] 4.2 Server policy and privilege tests pin the new policies, the caller setting and the functions-only table
- [x] 4.3 Kernel tests as the runtime role: a workspace transaction reads, changes and forges no key or counter; a secret is never stored and its replay is 409; ordinary replays work; limits still limit; both prunes work
- [x] 4.4 Server test: `register_client` through the BFF stores no secret and its replay answers 409
- [x] 4.5 `cargo xtask arch`: `secret_once` checks, the caller setting's name rule, a function's pinned `search_path`; unit tests

## 5. Wrap-up

- [x] 5.1 Rollback SQL checked against a database migrated to the base (identical columns, constraints, policies, grants and functions), and re-upgrading applies cleanly
- [x] 5.2 OpenAPI updated (five operations' `x-campbooks`), client types unchanged, events unchanged
- [x] 5.3 CHANGELOG updated
- [x] 5.4 Gates green (fmt, clippy, the whole workspace's tests, xtask arch / migrations / openapi / events / i18n, cargo deny)
