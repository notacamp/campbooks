# Tasks

## 1. Database

- [x] 1.1 Migration `20261113010000_identity__instance_apps_whole_row.sql`: the guard compares whole rows minus `last_used_at`
- [x] 1.2 Test: the runtime role cannot change any other column of an instance app, and cannot turn another client into one

## 2. Protocol

- [x] 2.1 `/oauth2/end_session`: an expired hint is accepted for 30 days after `exp`; test at 29 and 31 days
- [x] 2.2 `Clients::find`: a server not in development ignores an instance app's non-`https` redirect and post-logout URIs; tests in development and production

## 3. Docs

- [x] 3.1 CLI help and module docs: logging out of an instance app ends the Campbooks browser session
- [x] 3.2 Changelog entry
