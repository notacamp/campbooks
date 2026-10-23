# Tasks

## 1. The plan and its tooling

- [ ] 1.1 `tracking/tracking-plan.xlsx` with every sheet, dropdowns, the README, the fixed Actors rows, and the reference events
- [ ] 1.2 `cargo xtask tracking`: read (calamine), validate (every check in design.md), generate `tracking.gen.rs` per owning crate and `tracking/tracking-plan.json`; `--check`, `fmt` (rust_xlsxwriter, canonical), `apply <patch.yaml>`
- [ ] 1.3 xtask unit tests: each check fails on a fixture that breaks it; generation is deterministic

## 2. The kernel

- [ ] 2.1 Migration `kernel__tracked_events` (outbox, RLS like `kernel.events`) and `kernel__job_origin`
- [ ] 2.2 `tracking`: `TrackedEvent`, `Tracker::track`, `TrackingContext` (task-local), `Actor`, config `CAMPBOOKS_TRACKING__SINK` (`none`/`outbox`)
- [ ] 2.3 Jobs: store `origin` at enqueue and fan-out; the worker restores the context; payload audit lists `origin`
- [ ] 2.4 Testing helper: read back tracked events in a test database

## 3. The server

- [ ] 3.1 HTTP layer setting the context from the principal and the matched `operationId`; admin and anonymous routes
- [ ] 3.2 Compose the `Tracker` into `AppState`

## 4. Reference flow

- [ ] 4.1 `conversations.archive` / `unarchive` track their events with the conversation snapshot; Coverage rows for both

## 5. Workflow and docs

- [ ] 5.1 ADR 0020 and its README row
- [ ] 5.2 `docs/tracking.md` (how to add an event, the plan, the checks, the classification rules)
- [ ] 5.3 AGENTS.md "Tracked events (enforced)"; CONTRIBUTING; the PR template checkbox; OpenSpec config `design`/`tasks` rules; scaffold templates (`new-feature` adds a tracking reminder and a Coverage row)
- [ ] 5.4 CI: `cargo xtask tracking --check` in the Rust job

## 6. Tests

- [ ] 6.1 Rolled-back change leaves no event; another workspace's event is refused; sink `none` writes nothing
- [ ] 6.2 Archiving through the API writes one event with actor, operation, correlation and the snapshot
- [ ] 6.3 A delivery caused by a request carries `on_behalf_of` and the correlation

## 7. Last

- [ ] 7.1 OpenAPI regenerated (unchanged), client types unchanged
- [ ] 7.2 Changelog entry file (`changelog/unreleased/added/320-tracked-events.md`)
- [ ] 7.3 Gates green in CI; terminal recording of `cargo xtask tracking --check` and an archive producing its event
- [ ] 7.4 Archive the change (`openspec archive add-tracked-events --yes`)
