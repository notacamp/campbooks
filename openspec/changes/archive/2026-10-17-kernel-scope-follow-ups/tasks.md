# Tasks

## 1. Instance-wide rows

- [x] 1.1 Audit every UPDATE, DELETE, row lock and `ON CONFLICT` on `kernel.jobs` and `kernel.events` (worker completion, `mark_in`, `lift_for_instance_rows`, unique-key dedup, retries, reaping, release, pruning, admin queue, purge); list them in `design.md`
- [x] 1.2 `20261021010000_kernel__instance_rows_read_insert_only.sql`: `workspace_isolation` narrowed to the kernel scope or the own workspace; `instance_rows_read` (`FOR SELECT`) and `instance_rows_insert` (`FOR INSERT`) for a workspace transaction
- [x] 1.3 Tests as the runtime role: a workspace transaction cannot update, delete or lock instance-wide rows, nor move its own row out; it still reads and inserts them (unique-key duplicate, publish); instance-wide jobs still complete, retry, get reaped and fail for good

## 2. Arch-rule gaps

- [x] 2.1 One lexer for the text checks; C raw strings (`cr#"…"#`) read as strings
- [x] 2.2 A scope setting's last word on its own (`kernel_scope`, `maintenance_scope`) rejected in code and strings outside the kernel and the listed files, tests included
- [x] 2.3 `without_tests` cuts at the test module found in code only
- [x] 2.4 The BFF's `pool_access` entry narrowed to `store.rs` and `mod.rs`; a test that no server directory is exempt
- [x] 2.5 xtask unit tests for each; what a text check still cannot catch written down in `design.md`

## 3. The maintenance scope

- [x] 3.1 Kernel: `MAINTENANCE_SCOPE_SETTING` (crate-private), `begin_maintenance`, `JobContext::maintenance_tx`, `testing::maintenance_scope_setting`
- [x] 3.2 `20261021010100_connectors__maintenance_scope.sql` and `20261021010200_conversations__maintenance_scope.sql`: the maintenance policies trust the maintenance scope, not the kernel scope
- [x] 3.3 Connectors dispatcher and needs backfill dispatcher in the maintenance scope, each enqueue in its run's workspace
- [x] 3.4 `architecture.toml`: `[[access.maintenance_scope]]` for both, `[[access.kernel_scope]]` emptied; xtask checks the new door, setting, constant and word
- [x] 3.5 Tests: the maintenance scope reaches no kernel table; the dispatcher serves two workspaces; the kernel scope sees no backfill row; no module policy admits the kernel scope

## 4. Wrap-up

- [x] 4.1 Rollback SQL checked against a database migrated to the base
- [x] 4.2 OpenAPI unchanged (`cargo xtask openapi --check`), events unchanged (`cargo xtask events --check`)
- [x] 4.3 CHANGELOG updated
- [x] 4.4 Gates green (fmt, clippy, the whole workspace's tests, xtask arch / migrations / i18n, cargo deny)
