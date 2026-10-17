# Tasks

## 1. Callers

- [x] 1.1 List every `SECURITY DEFINER` function, its callers and what it runs as, and what each statement needs of which table; record them in `design.md`

## 2. The owner role

- [x] 2.1 Kernel migration: create `campbooks_definer` (NOLOGIN, no privileged attribute, no membership) once per cluster, check one created beforehand, let a non-superuser login take it, or stop naming the administrator's two statements
- [x] 2.2 Kernel migration: grant it schema `kernel` and the rate-limit counters' four privileges, hand over the two functions (lending `CREATE` on the schema for it), re-assert the grants
- [x] 2.3 BFF migration: grant it schema `bff` and the three tables' exact privileges, a policy for it alone on each, hand over the fifteen functions, re-assert the grants
- [x] 2.4 Checked on throwaway databases as a superuser, a `CREATEROLE` owner and a plain owner (with and without the role existing); every function works as the runtime role

## 3. Follow-ups (campbooks-cloud#214)

- [x] 3.1 `cargo xtask arch`: `pg_temp` followed by an identifier character, and a string naming `search_path`, are refused; unit tests
- [x] 3.2 `bff.start_login` checks the landing as the handler does
- [x] 3.3 `bff.touch_session`, `bff.store_session_tokens` and `bff.create_session` refuse missing values and out-of-bound deadlines; the absolute timeout is bounded to 366 days at startup
- [x] 3.4 The callback revokes the earlier session's refresh token
- [x] 3.5 The coverage tests see column grants (`has_any_column_privilege`)
- [x] 3.6 Item 7 noted in `design.md`

## 4. Tests

- [x] 4.1 Every definer function belongs to `campbooks_definer`, never a superuser or a login; the role's attributes, memberships, ownerships and privileges are pinned; the functions work as the runtime role
- [x] 4.2 The functions refuse bad input (landings, touch, store, create) and the session still works
- [x] 4.3 A column grant leaves a table reported open
- [x] 4.4 Signing in again revokes the earlier refresh token
- [x] 4.5 The absolute timeout's bounds are named at startup

## 5. Wrap-up

- [x] 5.1 Rollback SQL checked against a database migrated to the base (identical catalogue: owners, bodies, grants, policies, `_sqlx_migrations`), as a superuser and as a plain owner, and re-upgrading applies cleanly
- [x] 5.2 OpenAPI unchanged, client types unchanged, events unchanged
- [x] 5.3 CHANGELOG and `deploy/README.md` updated
- [x] 5.4 Gates green (fmt, clippy, the whole workspace's tests, xtask arch / migrations / openapi / events / i18n, cargo deny)
