# Tasks

## 1. Instance-wide rows from workspaces

- [x] 1.1 Audit every instance-wide insert reachable from a workspace transaction (publishes, enqueues, raw inserts in the kernel, the server and the modules); list them in `design.md`
- [x] 1.2 `DomainEvent::PUBLISHED_IN_WORKSPACES`, carried by the registry; `Registry::instance_rows_from_workspaces`; identity's `consent_revoked` and `way_in_removed` declare it
- [x] 1.3 `20261021040000_kernel__instance_rows_from_workspaces.sql`: the read-only list, `instance_rows_insert` limited to listed fresh events and deliveries, no schedule key outside the kernel scope
- [x] 1.4 The kernel refuses an instance-wide job, and an undeclared instance-wide event, from a workspace transaction
- [x] 1.5 Tests: each forgery refused (schedule key, the workspace's own schedule-keyed row, infinite `run_at`, a claimed or ordered delivery, an unlisted kind or event name, a future `occurred_at`); the legitimate shapes pass; identity's flows still publish; the server's test keeps the list equal to the registry

## 2. Stuck schedules

- [x] 2.1 The tick marks failed a schedule row of a workspace, due too late or leased too long, then enqueues
- [x] 2.2 `jobs::admin::discard_schedule` and `system.admin_discard_schedule_run` (`jobs.retry`), a live run kept
- [x] 2.3 Tests: pre-existing stuck rows healed, a retrying and a live row kept; the admin operation's permission, outcome and audit

## 3. The dispatchers

- [x] 3.1 `jobs::enqueue_in` / `enqueue_to_in`: set the workspace for one enqueue, clear it again
- [x] 3.2 The connectors and needs backfill dispatchers use them; tests that no workspace is set afterwards

## 4. Checks

- [x] 4.1 The server's policy test reads `USING` and `WITH CHECK` of every schema, the kernel's included, and pins where each scope appears
- [x] 4.2 `cargo xtask arch`: escapes decoded in literals, `admin_reads` bare word, the test-module cut only at the top level and at the end; unit tests
- [x] 4.3 The BFF's `mod.rs` loses its `pool_access` entry (its pool access moved into `store.rs`)

## 5. Wrap-up

- [x] 5.1 Rollback SQL checked against a database migrated to the base
- [x] 5.2 OpenAPI regenerated (the admin document), events unchanged
- [x] 5.3 CHANGELOG updated
- [x] 5.4 Gates green (fmt, clippy, the whole workspace's tests, xtask arch / migrations / openapi / events / i18n, cargo deny)
