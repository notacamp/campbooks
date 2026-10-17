# Tasks

## 1. Inventory

- [x] 1.1 List the instance-only kinds and the scope each runs in; check no code enqueues one with a workspace (`design.md`)
- [x] 1.2 Audit every consumer of `connectors.*`, `workspaces.*` and `identity.*` events: what it does and whether it reaches past the event's workspace (`design.md`)
- [x] 1.3 Check on Postgres 17 which statements of a workspace transaction need a read policy on instance-wide rows (`RETURNING`, `ON CONFLICT`, `xmin`)

## 2. Instance-only kinds

- [x] 2.1 `Job::INSTANCE_ONLY` / `JobHandler::INSTANCE_ONLY`; schedules and consumers of instance-wide events are instance-only; `Registry::instance_only_kinds`
- [x] 2.2 The worker refuses a workspace's job of an instance-only kind, failed for good, handler never run

## 3. Handlers re-check the state

- [x] 3.1 `identity.forget_connected_way_in`: `connection_of` before acting
- [x] 3.2 `identity.name_connected_mailbox`: names only the connection `connection_of` returns
- [x] 3.3 `identity.revoke_departed_consents` and `connectors.revoke_departed_grants`: `role_held` before acting
- [x] 3.4 `Membership::workspace_live`; `identity.remove_workspace_clients` and `connectors.revoke_workspace_grants` check it before acting

## 4. The tick's clock

- [x] 4.1 `tick_in` reads the tick from `clock_timestamp()`

## 5. Workspaces read no instance-wide row

- [x] 5.1 `EventBus::publish` appends an instance-wide event under an id it chooses, reading nothing back
- [x] 5.2 `20261021170000_kernel__instance_rows_unreadable_from_workspaces.sql` drops `instance_rows_read`, 5 s lock timeout, one transaction
- [x] 5.3 Rollback SQL checked against a database migrated to the base

## 6. Tests

- [x] 6.1 Kernel: a workspace's `kernel.prune_jobs` row is refused and prunes nothing; the scheduled one prunes; the registry lists the instance-only kinds
- [x] 6.2 Kernel: a stalled tick never moves a newer last tick back
- [x] 6.3 Kernel: a workspace transaction reads no account event or instance-wide delivery, still publishes a listed instance-wide event with its deliveries; the kernel scope reads them all
- [x] 6.4 Identity: a forged `connection_removed` for a live connection, and one the port cannot answer for, change nothing; a forged `connection_added` names nothing
- [x] 6.5 Identity: a forged `member_left` (still a member) and `workspace_deleting` (still live) revoke nothing; the real ones still do
- [x] 6.6 Connectors: the same for grants at the provider

## 7. Wrap-up

- [x] 7.1 OpenAPI and events unchanged; no client change
- [x] 7.2 CHANGELOG updated
- [x] 7.3 Gates green (fmt, clippy, the whole workspace's tests, xtask arch / migrations / openapi / events / i18n, cargo deny)
