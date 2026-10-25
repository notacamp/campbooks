# Tasks

## 1. The plan

- [ ] 1.1 The plan tool knows `bff` as a platform module (generated into the kernel, tracked from `crates/server/`)
- [ ] 1.2 `tracking/patches/0004-bff.yaml`: Changelog row (plan version 4), the 3 events, their properties, the enums `bff.web_session_end_cause` and `bff.web_sessions_end_cause`, and a Coverage row for every mutating operation, state-changing read, handler and schedule of the BFF
- [ ] 1.3 `apply`, `generate`, `check`; commit the workbook, the mirror and the generated code

## 2. Tracking

- [ ] 2.1 `bff::store` opens the transaction a session starts or ends in (`begin`); `insert_session`, `delete_session` and `delete_for_logout` take a connection
- [ ] 2.2 Callback: `web_session_started`, and `web_session_ended` (`replaced`) for the browser's previous session
- [ ] 2.3 Logout (`logout`) and the lookups and refreshes that end a session (`copied_cookie`, `scopes_outdated`, `refresh_refused`, `tokens_unreadable`, `access_expired`)
- [ ] 2.4 Back-channel logout and the two handlers: `web_sessions_ended`

## 3. Tests

- [ ] 3.1 The BFF test harness writes the outbox
- [ ] 3.2 A test per live event and cause reading it back from `kernel.tracked_events` (name, version, actor, workspace, operation, body), that a no-op tracks nothing, and that no event holds an address or a name
- [ ] 3.3 A call through `/bff/api/*` is tracked in the operation reached, as the person; one through `/bff/admin/*` as the administrator, and the handler it causes on their behalf with the same correlation
- [ ] 3.4 The server's coverage test lists nothing of `bff`

## 4. Last

- [ ] 4.1 OpenAPI regenerated (unchanged), client types unchanged
- [ ] 4.2 Tracking plan regenerated
- [ ] 4.3 Changelog entry file (`changelog/unreleased/added/327-track-bff.md`)
- [ ] 4.4 Gates green in CI
