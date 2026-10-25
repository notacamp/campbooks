# Tasks

## 1. Kernel and schedule

- [x] 1.1 Kernel: `Holds` port (`HoldRequest`, `Held`, `hold`, `release`), `NoHolds` (Unavailable), `AppState::with_holds`; unit test of the default
- [x] 1.2 Schedule: implement `Holds` reusing `create_event`/`delete_event` internals (writable member calendar, outbound for providers, publish in the same tx); register in the server; tests: Campbooks calendar, provider calendar (outbound requested), another member's calendar (not found), read-only (refused), release no-op cases

## 2. Tasks server

- [x] 2.1 Migration (tasks schema, slot after the newest migration on `next`): `hold_event`, `held_from`, `held_to` + CHECK all-or-none + partial unique index; `TaskRow`/`TaskView.hold`
- [x] 2.2 `tasks.hold` and `tasks.release_hold` (errors, revision, plan day via `TimeZones`, history `held`/`released`, `task_changed` with `planned`, undo declarations with restoring rows); tests incl. move, undo, pay refused, stale revision, not the caller's
- [x] 2.3 Done / not needed / hand-off release a hold not yet started; tests
- [x] 2.4 Consumer of `schedule.event_created|changed|cancelled` for `tasks.task` sources (actor `calendar`, idempotent); `architecture.toml` consumers; tests for move, remove, restore, all-day, unknown task
- [x] 2.5 Tracking plan: `task_held`, `task_hold_released`, Coverage rows for the two operations and the handler; regenerate

## 3. Client

- [x] 3.1 OpenAPI + client types regenerated
- [x] 3.2 `tasks`: `hold`/`release` in `useTaskActions` (optimistic, Undo); `HoldTime` panel (three slots, Length, Calendar, none-free line) fed through app-shell plugs for free slots and writable calendars; held chip on row and detail; Release; `h` key; no Hold for Pay
- [x] 3.3 Home: Hold time (`h`) on Do task rows in a dialog with the same panel; held chip
- [x] 3.4 Calendar: no Planned row for a held task; no Due row on its held due day; Hold time (`h`) in the Do task sheet
- [x] 3.5 Strings in en, pt-PT, es-ES and fr (copy fits at 375 px)
- [x] 3.6 Stories (task detail holding, Home hold dialog, calendar with a held task) with a play-less one for the recording; route/unit tests

## 4. Before merge

- [x] 4.1 Changelog entry file (`changelog/unreleased/added/`); `openspec validate hold-time-for-a-task --strict`; gates green in CI
- [ ] 4.2 Recording (375 px and desktop), then archive this change
