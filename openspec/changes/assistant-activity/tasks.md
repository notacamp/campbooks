# Tasks

Slice 1 of the design (Decision 6). Corrections, the new Visor emotions and
reasoning steps are their own changes.

## 0. Before building

- [ ] 0.1 Owner sign-off on the design (gated)
- [ ] 0.2 Visual prototype of the list, the running session and the phone screens (artifact), approved before any UI task

## 1. Kernel port

- [ ] 1.1 `kernel::activity`: `Activity` trait, `Entry`, `Target`, `SessionSpec`, `Subject`, `Step`, `Facts` (v1), `Trigger`, `Audience`, `StepState`; `NoActivity` default; `AppState::with_activity`
- [ ] 1.2 `cargo xtask arch`: a recorded kind must be prefixed by the recording module (static check on `SessionSpec { kind: … }` literals) and documented in the module's descriptor
- [ ] 1.3 ADR 0015 (assistant activity: a personal-data-free journal behind a kernel port, live through the change feed)

## 2. `activity` module

- [ ] 2.1 `cargo xtask new-module activity`; migration `<ts>_activity__init.sql` (sessions, subjects, steps, indexes, forced RLS, grants); scope `activity:read`; config `CAMPBOOKS_ACTIVITY__RETENTION_DAYS`, `__STALL_MINUTES`
- [ ] 2.2 Port implementation: new / by id / covering-subject / failed targets, pending stages, working → done updates, outcome counts, status transitions; `activity.session_changed` v1 with `changes()`; DB tests incl. rollback and concurrent steps
- [ ] 2.3 `activity.list_sessions` (cursor, limit, status filter, visibility via the `Visibility` port per distinct container, member-private rule); tests incl. non-member 404 and admin
- [ ] 2.4 `activity.get_session` (`after`, `limit`); 404 rules; tests
- [ ] 2.5 Schedules `activity.prune` and `activity.sweep_stalled`; consumers of `workspaces.workspace_deleting` (purge after grace) and `workspaces.member_left`; tests
- [ ] 2.6 Register the module and the port in the server; scopes in the catalogue; telemetry privacy test covers an activity-recording job

## 3. `conversations`

- [ ] 3.1 `apply_changes`: one triage session per batch with inbound mail; rules steps with facts and decision refs; pending model stage when the role is configured and the message is in the window; failed session on the last attempt
- [ ] 3.2 `decide_message`: working step in its own transaction before the call; done in the job transaction with the answers; skipped / failed (last attempt, error code via `redact`); nothing on a replay with nothing pending
- [ ] 3.3 `conversations.list_decisions` (1 to 100 ids, inbox visibility, answers with evidence, corrections); tests
- [ ] 3.4 Tests: a batch with the fake laya-serve fills one session end to end; without the role; outbound only; the default port changes nothing

## 4. Client

- [ ] 4.1 `modules/activity`: API hooks (generated types), `records.gen.ts` mapping, 250 ms coalesced invalidation, incremental step fetch
- [ ] 4.2 Session list (Segmented filters, live rows, progress, "N new", J/K/Enter) and session timeline (header, groups, stage lines, Why? from `conversations` via its `index.ts`, Open conversation O, Esc)
- [ ] 4.3 Shell: sidebar item with the live Visor, phone top-bar Visor button, `nav.activity` (G A) and `activity.open_conversation` (O) in the registry with the conflict check; reader Why? link "See what Campbooks did"
- [ ] 4.4 Empty, no-AI, missing-scope and unavailable-message states; `aria-live` announcements
- [ ] 4.5 i18n namespace `activity` in en, pt-PT, es-ES, fr (full diacritics, European Portuguese, Spanish from Spain); any new `lib/ui` piece with story and test in light and dark
- [ ] 4.6 Recording scenario: a fake Gmail batch arriving and its session filling in live, Why?, Open conversation

## 5. Docs and finish

- [ ] 5.1 `docs/domain-map.md` (activity context: language, publishes, consumes), `docs/architecture.md` (the port, the module), `deploy/.env.example`
- [ ] 5.2 OpenAPI regenerated (`cargo xtask openapi`), events catalogue regenerated, `.sqlx` regenerated, client types regenerated (`pnpm api:check`)
- [ ] 5.3 CHANGELOG `[Unreleased]`
- [ ] 5.4 Gates green (Rust, client, OpenSpec strict); UI verified at 375px and desktop; recording uploaded
- [ ] 5.5 Archive the change (`openspec archive assistant-activity --yes`) in the same PR
