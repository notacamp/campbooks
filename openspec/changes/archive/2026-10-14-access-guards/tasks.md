# Tasks

## 1. Kernel
- [x] 1.1 `authz`: catalogue, `authorize`/`require`/`require_type`/`require_account`, `decide`, tests
- [x] 1.2 `Module.resources`, `Registry::resources`, `AppState::with_resources`, `Operation::access`
- [x] 1.3 Kernel RLS migration; `publish` refuses another workspace's event; isolation test
- [x] 1.4 Change audience, per-page visibility, `reset`/`access_changed`, `Visibility::access_records`
- [x] 1.5 Replay authorization (`checked_container`, scope, membership, containers), test
- [x] 1.6 `BlobStore::detach_owners`

## 2. Modules
- [x] 2.1 workspaces: catalogue and facts, handlers on `authorize`, `remove_member ?mail=`, `member_left.mail`
- [x] 2.2 connectors: catalogue and facts, owner-only disconnect, `?mail=`, `connection_removed` contract, audiences, schedule policies
- [x] 2.3 conversations: catalogue and facts, handlers on `authorize`, `forget_connection`, `forget_member`, inbox owner and `detached_at`, tombstones, draft audiences, `read_only`
- [x] 2.4 search: `search.result`, `on_inbox_removed`, cursor bound to workspace, `indexing` only with containers
- [x] 2.5 identity, system, admin, server (changes, BFF): resource types; identity bearer handlers on `require_account`

## 3. Structural guards
- [x] 3.1 `cargo xtask arch`: access declarations, `authorize` calls, workspace-table queries, unconsumed events; `architecture.toml` exceptions
- [x] 3.2 Access matrix tests (catalogue × principals × actions; operations × callers), feed tests, RLS coverage incl. kernel, maintenance policy test

## 4. Finish
- [x] 4.1 ADR 0017, architecture docs, AGENTS.md rule
- [x] 4.2 OpenAPI, events catalogue, client types regenerated; `.sqlx` regenerated
- [x] 4.3 CHANGELOG updated
- [x] 4.4 Gates green; recording of the matrix test and the purge flows
