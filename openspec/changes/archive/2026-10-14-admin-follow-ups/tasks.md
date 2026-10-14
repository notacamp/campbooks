# Tasks

## 1. Suspended badge

- [x] 1.1 identity: `admin_list_accounts` (`GET /admin/v1/identity/accounts?ids=`, `users.read`, ≤200 ids, `too_many_accounts` in four locales)
- [x] 1.2 Tests: id and suspension time only, unknown and non-UUID ids left out, 400 beyond 200, 404 for non-administrators
- [x] 1.3 Client: identity `useSuspendedAccounts` / `SuspendedAccounts` (one request per page of 50), invalidated by suspend/unsuspend; People list `Tag` "Suspended" (four locales); test

## 2. Job errors

- [x] 2.1 `redact::job_error` (first line, quotes, addresses, URLs, opaque runs, control characters) + unit tests
- [x] 2.2 Worker stores and logs the sanitized message, bounded to 500 characters; commit failures via `redact::sqlx`
- [x] 2.3 Test: a handler quoting a provider reply with a subject, an address, a link and a body leaves none of it in `last_error`

## 3. Codes copy

- [x] 3.1 Codes lead in en, pt-PT, es-ES, fr; comments; CHANGELOG

## 4. Close

- [x] 4.1 OpenAPI + client types regenerated; gates; recordings (UI 375 px + desktop, terminal clip of the job-error test)
- [x] 4.2 Archive this change
