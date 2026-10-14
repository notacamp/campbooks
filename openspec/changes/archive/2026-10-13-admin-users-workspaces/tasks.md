# Tasks

## 1. Workspaces

- [x] 1.1 Migration `workspaces__admin_reads`: `FOR SELECT` admin-read policies on `workspaces`, `members`, `invitations`; users index for paging
- [x] 1.2 `domain/admin_page`: `q`/`after`/`limit` checking, opaque cursor, literal prefix; unit tests; reasons `invalid_cursor`, `query_too_long` in four locales
- [x] 1.3 `admin_list_users`, `admin_get_user`, `admin_list_workspaces`, `admin_get_workspace` + tests (permissions, 404s, paging, search, no addresses of invitees)

## 2. Identity

- [x] 2.1 `domain/suspension` (`suspend`/`unsuspend`, idempotent, audit, `identity.account_suspended` event)
- [x] 2.2 `admin_get_account`, `admin_suspend_account` (refuses an admin profile via `AdminProfiles`, `account_is_admin` in four locales), `admin_unsuspend_account` + tests
- [x] 2.3 Sign-in refusal `account_suspended`; end-to-end test of every door (provider sign-in, issuer session, code, refresh) and unsuspending

## 3. Connectors

- [x] 3.1 Migration `connectors__admin_reads` (`connections`, `streams`)
- [x] 3.2 `admin_list_connections` + tests (no credential, cursor or scopes; only the named workspace)

## 4. Server

- [x] 4.1 `bff.end_suspended_sessions` consumer + test
- [x] 4.2 Admin-read allowlist gains the five tables

## 5. Client

- [x] 5.1 Admin → People: list (search, show more), person page (account, ways in, sessions, workspaces), Suspend/Unsuspend with inline confirmation
- [x] 5.2 Admin → Workspaces: list (search, show more), workspace page (members, connection health)
- [x] 5.3 Sign-in screen `account_suspended`; i18n ×4; tests

## 6. Last

- [x] 6.1 OpenAPI (`admin.json`, `events.json`) and client types regenerated; `.sqlx` regenerated
- [x] 6.2 CHANGELOG updated
- [x] 6.3 Gates green; UI verified at 375 px and desktop; recordings
