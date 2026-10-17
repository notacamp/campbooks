# Tasks

## 1. Inventory

- [x] 1.1 List the key indexes, every `ON CONFLICT` target, the ordering-key claim check and lock, and every caller passing a unique or ordering key, with the scope each runs in; check none relies on cross-workspace dedup (`design.md`)

## 2. Keys per workspace

- [x] 2.1 `20261021130000_kernel__job_unique_keys_per_workspace_index.sql`: `(kind, unique_key, workspace_id) NULLS NOT DISTINCT`, built concurrently
- [x] 2.2 `20261021130100_kernel__job_ordering_keys_per_workspace_index.sql`: `(ordering_key, workspace_key, seq)`, built concurrently
- [x] 2.3 `20261021130200_kernel__job_keys_per_workspace.sql`: drop the old two indexes, 5 s lock timeout
- [x] 2.4 `jobs::insert_row` and `EventBus::replay` conflict on the new index; the enqueue's advisory lock is per workspace and key
- [x] 2.5 The claim's ordering check compares the workspace
- [x] 2.6 `jobs::admin::retry` / `retry_kind`: a key is held only within its workspace

## 3. Tests

- [x] 3.1 Two workspaces enqueue the same unique key and both run; within one workspace and among instance-wide jobs it still dedupes
- [x] 3.2 Ordering keys of two workspaces and of instance-wide jobs don't serialize each other, at claim or at enqueue; within one workspace they still run in order
- [x] 3.3 As the runtime role, a workspace's own row with another workspace's key no longer stops that workspace's job
- [x] 3.4 Connectors: another workspace's row with a stream's `connectors.sync_stream:<stream>` key no longer stops the stream's dispatched run
- [x] 3.5 Admin: a dead job's key is held only by its own workspace's jobs
- [x] 3.6 The schedule tick still runs once per period (existing tests), and #207's guards still hold (existing tests)

## 4. Wrap-up

- [x] 4.1 Rollback SQL checked against a database migrated to the base
- [x] 4.2 OpenAPI and events unchanged; no client change
- [x] 4.3 CHANGELOG updated
- [x] 4.4 Gates green (fmt, clippy, the whole workspace's tests, xtask arch / migrations / openapi / events / i18n, cargo deny)
