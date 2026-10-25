# Tasks

## 1. The plan

- [ ] 1.1 `tracking/patches/0010-tasks.yaml`: Changelog row (plan version 10), the 9 events, their properties, the records `tasks.task`, `tasks.task_due` and `tasks.task_plan`, the enums, and a Coverage row for every mutating operation, job and handler of the module
- [ ] 1.2 `apply`, `generate`, `check`; commit the workbook, the mirror and the generated code; `crates/modules/tasks/src/tracking.gen.rs` included from `lib.rs`

## 2. Tracking

- [ ] 2.1 `domain/tracked.rs`: the snapshot of a task from its row, the plan's enums, and the before/after comparison that picks the events
- [ ] 2.2 `domain::finish` tracks every change; every caller passes the row before (`Outcome::before`)
- [ ] 2.3 `add_read_asks`: created, and due changed (the task locked and read before the update)

## 3. Tests

- [ ] 3.1 The test kit writes the outbox on demand (`Kit::tracking`) and invokes an operation as the assistant
- [ ] 3.2 A test per live event reading it back from `kernel.tracked_events` (name, version, actor, workspace, operation, snapshot), that a no-op tracks nothing, and that no event holds a title, an address or an amount
- [ ] 3.3 The server's coverage test lists nothing of `tasks`

## 4. Last

- [ ] 4.1 OpenAPI regenerated (unchanged), client types unchanged
- [ ] 4.2 Tracking plan regenerated
- [ ] 4.3 Changelog entry file (`changelog/unreleased/added/340-track-tasks.md`)
- [ ] 4.4 Gates green in CI
