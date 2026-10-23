## 0. Design gate

- [x] 0.1 Proposal, design with alternatives, spec deltas, visual prototype
- [x] 0.2 Owner sign-off on the prototype (2026-10-23) and the AI rule (one Jev pass + at most one chat read per email)
- [x] 0.3 Seams agreed with schedule (#42) and Scout chat (#305) sessions

## 1. Module and schema

- [x] 1.1 Crate `crates/modules/tasks` following `attention`'s layout (lib.rs `module()`, domain/, features/, events.rs, scopes.rs, testkit.rs, i18n in four locales); registered in root `Cargo.toml`, `crates/server/Cargo.toml` and `modules_with`
- [x] 1.2 Migration `<ts>_tasks__init.sql`: `tasks.tasks` and `tasks.history` with forced RLS, `workspace_isolation`, grants and indexes as in design.md
- [x] 1.3 Resource type `tasks.task` in the ADR 0017 catalogue (assignee owns; `handed_by` reads and takes back) with `ResourceFacts`; `cargo xtask arch` rules met
- [x] 1.4 Workspace deletion handler; test

## 2. Operations

- [x] 2.1 `tasks.create` (title cleaning, source read check through `authz`, `source_container`/`source_sender` from the source's facts); tests incl. blank title 400 and unseen source 404
- [x] 2.2 `tasks.list` (filters, `ids` 1–100, order, caller's UTC offset) and `tasks.get` (with history); tests
- [x] 2.3 `tasks.update`, `tasks.plan` (`none`|`date`|`later`), `tasks.done`, `tasks.not_needed`, `tasks.reopen` with revision 409s, history and `x-campbooks` undo; tests incl. done → undo
- [x] 2.4 `tasks.hand_off` / `tasks.take_back` (members only, `handed_by`); access tests (third member 404)
- [x] 2.5 `tasks.task_changed.v1` published in every write's transaction; contract JSON schema in `contracts/`; payload gate passes (no free text)

## 3. Attention

- [x] 3.1 Container types beyond `conversations.inbox` through the `Visibility` port; record type no longer hard-coded; tasks answers `tasks.assignee:<member>`
- [x] 3.2 `attention.project_task`: upsert by record, newer revisions only, tombstones, clear on done/not needed/hand-off; order-proof test over random delivery orders
- [x] 3.3 Later: items hidden until `later_until`, back by themselves; test
- [x] 3.4 A task supersedes its source conversation's `do`/`pay` item in `attention.list`; test

## 4. Ports and Scout

- [x] 4.1 `OpenTasks` kernel port (`ws, inbox, conversation, sender_address` → ≤10) implemented by tasks; test incl. the other-inbox case
- [x] 4.2 If campbooks-cloud#305 phase 1 is on `next`: anchor registry `tasks.task` → `tasks.get`, tools `my_tasks` / `read_task` (≤8 KiB, RFC 3339, RecordRefs, titles never logged), `crates/modules/tasks/handbook.md` as `pub const HANDBOOK`; otherwise note it on #305

## 5. Client

- [x] 5.1 Module `clients/app/src/modules/tasks` (routes, nav item with count, `g t`, `n t` global sequence, i18n en/pt-PT/es-ES/fr)
- [x] 5.2 Tasks place: groups, rows, quick add, detail pane, Later options, hand-off filter by name, keys, undo toasts, failure rollback; Storybook screen story with mock fetch
- [x] 5.3 Home: task items in Do and Pay via `tasks.list?ids=` with Done/Paid, Later, Not needed
- [x] 5.4 Reader: "Make it a task" (`n t`) inline form
- [x] 5.5 `pnpm test:fit` passes at 375 and 1280 in every locale

## 6. Finish

- [x] 6.1 OpenAPI regenerated (`cargo xtask openapi`, events) — from CI's `generated-openapi` artifact
- [x] 6.2 Client types regenerated (`pnpm --dir clients api:generate`) — from CI's `generated-api-types` artifact
- [x] 6.3 Changelog entry `changelog/unreleased/added/43-tasks.md`
- [x] 6.4 Gates green in CI
- [x] 6.5 UI verified and recorded at 375 px and desktop (Storybook screen stories)
- [x] 6.6 `openspec archive add-tasks-module --yes` before merge
