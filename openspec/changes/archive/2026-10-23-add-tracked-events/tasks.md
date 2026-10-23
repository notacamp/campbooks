# Tasks

## 1. The plan and its tooling

- [x] 1.1 `tracking/tracking-plan.xlsx` with every sheet, dropdowns, the README, the fixed Actors rows, and the reference events (`init`, then `apply tracking/patches/0001-initial.yaml`)
- [x] 1.2 `crates/tracking-plan` (`campbooks-tracking-plan`, lib + bin, no dependency on the kernel or the server): read (calamine), validate (every check in design.md), generate `tracking.gen.rs` per owning crate and `tracking/tracking-plan.json`; `check`, `generate`, `fmt` (rust_xlsxwriter, canonical, byte-stable), `apply <patch.yaml>`, `init`; `cargo xtask tracking` wraps it; `cargo xtask arch` knows it as a tooling crate
- [x] 1.3 Unit tests: each check fails on a fixture that breaks it; generation and the workbook are deterministic; a workbook written then read gives the plan back

## 2. The kernel

- [x] 2.1 Migrations `kernel__tracked_events` (outbox, RLS like `kernel.events`, append only), `kernel__job_origin` and its `_validate`
- [x] 2.2 `tracking`: `TrackedEvent`, `Tracker::track`, `TrackingContext` (task-local), `Actor`, config `CAMPBOOKS_TRACKING__SINK` (`none`/`outbox`)
- [x] 2.3 Jobs: store `origin` at enqueue and fan-out; the worker restores the context; payload audit lists `origin` (`JOB_ORIGIN_FIELDS`)
- [x] 2.4 Testing helpers: `testing::tracking_outbox`, `testing::tracked_events`

## 3. The server

- [x] 3.1 HTTP layer setting the context from the principal and the matched `operationId` (in the kernel, applied by `operations::compose` and `testing::app`; the server applies it to its admin, BFF and issuer routes); admin and anonymous routes
- [x] 3.2 Compose the `Tracker` into `AppState` (from the config; the server names its role)
- [x] 3.3 Coverage test against the real registry and catalogue (`REQUIRE_FULL_COVERAGE = false` until #332); the grant tests know the append-only outbox

## 4. Reference flow

- [x] 4.1 `conversations.archive` / `unarchive` track their events with the conversation snapshot; Coverage rows for both

## 5. Workflow and docs

- [x] 5.1 ADR 0020 and its README row
- [x] 5.2 `docs/tracking.md` (how to add an event, the plan, the checks, the classification rules)
- [x] 5.3 AGENTS.md "Tracked events (enforced)"; CONTRIBUTING; the PR template checkbox; OpenSpec config `design`/`tasks` rules; scaffold templates (`new-feature` adds a tracking reminder and the Coverage TODO; `new-module`'s design and tasks ask for them)
- [x] 5.4 CI: `cargo run -p campbooks-tracking-plan -- check` in the Rust job

## 6. Tests

- [x] 6.1 Rolled-back change leaves no event; another workspace's event is refused; sink `none` writes nothing; the runtime role only appends
- [x] 6.2 Archiving through the API writes one event with actor, operation, correlation and the snapshot; the assistant's archive is on the member's behalf
- [x] 6.3 A delivery caused by a request carries `on_behalf_of` and the correlation

## 7. Last

- [x] 7.1 OpenAPI regenerated (unchanged), client types unchanged
- [x] 7.2 Changelog entry file (`changelog/unreleased/added/320-tracked-events.md`)
- [x] 7.3 Gates green in CI; terminal recording of `cargo run -p campbooks-tracking-plan -- check` and an archive producing its event; tracking plan regenerated
- [x] 7.4 Archive the change (`openspec archive add-tracked-events --yes`)
