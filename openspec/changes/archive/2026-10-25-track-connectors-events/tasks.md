# Tasks

## 1. The plan

- [ ] 1.1 `tracking/patches/0006-connectors.yaml`: Changelog row (plan version 6), the 10 events, their properties, the records `connectors.connection`, `connectors.stream` and `connectors.send`, the enums, and a Coverage row for every mutating operation, job, schedule and handler of the module (and the identity operations that hand it a grant)
- [ ] 1.2 `apply`, `generate`, `check`; commit the workbook, the mirror and the generated code; `crates/modules/connectors/src/tracking.gen.rs` included from `lib.rs`

## 2. Tracking

- [ ] 2.1 `domain/tracked.rs`: snapshots of a connection, a stream and a send from the module's rows, and the plan's enums
- [ ] 2.2 Connections: added and reconnected in `grants::save`; removed in `removal::delete` with its cause
- [ ] 2.3 Health: `health::set` and `health::needs_reauth` track health changes (sync, send, IDLE)
- [ ] 2.4 Sync now: `connection_sync_requested`
- [ ] 2.5 Sync runs: `stream_synced` (mail and calendar), `outbound_changes_made` (Gmail, Zoho, IMAP), `calendar_change_completed` (the run and `record_calendar_change`)
- [ ] 2.6 Sends: `send_succeeded`, `send_failed`

## 3. Tests

- [ ] 3.1 The module's test kit writes the outbox
- [ ] 3.2 A test per live event reading it back from `kernel.tracked_events` (name, version, actor, workspace, operation, snapshot), that a no-op tracks nothing, and that no event holds an address or a name
- [ ] 3.3 The server's coverage test lists nothing of `connectors`

## 4. Last

- [ ] 4.1 OpenAPI regenerated (unchanged), client types unchanged
- [ ] 4.2 Tracking plan regenerated
- [ ] 4.3 Changelog entry file (`changelog/unreleased/added/329-track-connectors.md`)
- [ ] 4.4 Gates green in CI
