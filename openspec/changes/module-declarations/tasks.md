# Tasks

## 1. Kernel declarations

- [x] 1.1 `RecordRef` (serde with validated type, utoipa schema), `RecordType`, `Access`, `RecordTypeDecl`; verify with unit tests
- [x] 1.2 `Action` type-state builder, `Undoable`, `ActionDecl`, `ActionContext` with the lazy shared transaction; verify with unit tests
- [x] 1.3 `Declarations` (records, actions, events) on `Module`; `system` and test fixtures updated
- [x] 1.4 `Catalogue::build` with every consistency rule and its JSON shape; verify with unit tests over a broken test module

## 2. Generated HTTP surface

- [x] 2.1 Migration `20261013120000_kernel__actions.sql` (`idempotency_keys`, `undo_tokens`); `kernel.prune_actions` scheduled hourly; verify pruning with a DB test
- [x] 2.2 `declare::http::routes`: one OpenAPI operation per action, scope via the shared `auth::authorize`, body validation (400/413/415/422); verify with DB tests
- [x] 2.3 Idempotency: claim/replay/mismatch, rollback on failure, concurrent duplicates act once; verify with DB tests
- [x] 2.4 Undo tokens: issue in the action's transaction, redeem once, caller/workspace binding, scope, expiry; verify with DB tests
- [x] 2.5 Server: action routes in `api()`, `catalogue()`, `campbooks-server declarations`, startup check; verify the OpenAPI lists every action with its scope and schemas

## 3. Catalogue and rules

- [x] 3.1 `cargo xtask declarations [--check]`, `openapi/declarations.json`, CI step
- [x] 3.2 `cargo xtask arch`: declaration errors, action scope and schemas, a test per action, reserved `actions`; verify with xtask unit tests and by removing the `system.whoami` test literal
- [x] 3.3 Reference action `system.whoami` with tests calling it by name

## 4. Follow-ups (not in this change)

- [ ] 4.1 `ActionContext::workspace()` and `campbooks.workspace_id` on the action's transaction (with #18)
- [ ] 4.2 Client: generate the shortcut registry and command menu from `openapi/declarations.json` (design-system change)
- [ ] 4.3 MCP tools, assistant tools and automation generated from the catalogue
- [ ] 4.4 Check that every catalogued label key exists in the client i18n files

## 5. Wrap-up

- [x] 5.1 `docs/architecture.md`, `AGENTS.md` (adding an action), `CONTRIBUTING.md` updated
- [x] 5.2 OpenAPI regenerated; client types regenerated
- [x] 5.3 CHANGELOG `[Unreleased]` updated
- [x] 5.4 Gates green: fmt, clippy, tests, `cargo xtask arch`, `cargo xtask openapi --check`, `cargo xtask declarations --check`, sqlx prepare check, `cargo deny`, `cargo machete`, client gates, `openspec validate --all --strict`
- [x] 5.5 UI: none
