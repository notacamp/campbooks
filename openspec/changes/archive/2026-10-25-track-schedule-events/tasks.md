# Tasks

## 1. The plan

- [x] 1.1 `tracking/patches/0009-schedule.yaml`: Changelog row (plan version 9), the 16 events, their properties, the records `schedule.calendar_event`, `schedule.calendar_event_times`, `schedule.calendar` and `schedule.settings`, the enums, and a Coverage row for every mutating operation, job and handler of the module
- [x] 1.2 `apply`, `generate`, `check`; commit the workbook, the mirror and the generated code; `crates/modules/schedule/src/tracking.gen.rs` included from `lib.rs`

## 2. Tracking

- [x] 2.1 `domain/tracked.rs`: snapshots of an event, a calendar and the settings from the module's rows, the plan's enums, and the before/after comparison of an event (`changes`)
- [x] 2.2 Events: created (`create_event`), edited and moved (`update_event`), deleted, restored, responded, pushed (`push_row`), unpushed
- [x] 2.3 Calendars and settings: enabled, disabled, recoloured (`update_calendar`), settings updated
- [x] 2.4 Handlers: `add_read_items` (created, edited, moved, cancelled, pushed), `grant_calendars` (calendar added), `apply_calendar_changes` (one summary per batch, counting removals and a full read's leftovers too), `forget_connection` (calendar removed)

## 3. Tests

- [x] 3.1 A test kit for the module (`testkit.rs`) writing the outbox
- [x] 3.2 A test per live event reading it back from `kernel.tracked_events` (name, version, actor, workspace, operation, snapshot), that a no-op tracks nothing, and that no event holds a title, a place or notes
- [ ] 3.3 The server's coverage test lists nothing of `schedule`

## 4. Last

- [x] 4.1 OpenAPI regenerated (unchanged), client types unchanged
- [x] 4.2 Tracking plan regenerated
- [x] 4.3 Changelog entry file (`changelog/unreleased/added/349-track-schedule.md`)
- [ ] 4.4 Gates green in CI
