# Tasks

Builders don't compile or test locally: push and read CI. One builder at a
time.

## 1. Scaffold

- [ ] 1.1 `schedule` module from the module template, written by hand: crate `campbooks-schedule`, `scopes.rs` (`schedule:read`/`write`), `events.rs`, `features/`, migration `<ts>_schedule__init.sql` (schema `schedule`, RLS on `workspace_id`), client module `clients/app/src/modules/schedule` with 4 locale files
- [ ] 1.2 Register the module in `crates/server` and `clients/app/src/app/modules.ts`; `architecture.toml` needs no entry
- [ ] 1.3 `openspec validate add-schedule-module --strict` passes

## 2. Contracts and ports

- [ ] 2.1 `contracts/conversations.message_read.v1.schema.json` (ids and enums only; tasks session reviews)
- [ ] 2.2 `contracts/connectors.calendar_changed.v1.schema.json` + the calendar event blob schema
- [ ] 2.3 `contracts/schedule.event_created|event_changed|event_cancelled.v1.schema.json`
- [ ] 2.4 `contracts/schedule.outbound_requested.v1.schema.json` + its blob; the outcome event for calendar changes
- [ ] 2.5 `capability_granted` calendar blob (calendar list)
- [ ] 2.6 Kernel ports: `OpenTasks::open_tasks(workspace, inbox_id, conversation_id, sender_address)` (no implementation; empty list), `TimeZones`, `Availability::free_slots(member, window, duration)`, the outbound-requests port for schedule
- [ ] 2.7 `openapi/events.json` lists the new events (`cargo xtask events` runs in CI)

## 3. Decide pass (L1)

- [ ] 3.1 `has_dated_thing` (noul) and `dated_kind` (choice) in `triage_questions()`; recorded only (`Policy::baseline` treats them as unmeasured)
- [ ] 3.2 Flags: `CAMPBOOKS_CONVERSATIONS__DATED__FLAG` (0.6) and `__DATED__ENABLED`; a flagged marker on the message; tests for newsletters and suspicious
- [ ] 3.3 Answers-check fixtures updated for the ten questions

## 4. Structured data (L0)

- [ ] 4.1 iCalendar reader (a maintained crate, `cargo deny` clean): VEVENT → item, METHOD:CANCEL, SEQUENCE
- [ ] 4.2 schema.org reader (JSON-LD and microdata) for the listed types only
- [ ] 4.3 DMARC-aligned + not-suspicious gate; read stored with layer `structured`; `message_read` published
- [ ] 4.4 Test: the structured reader never touches the text part or prose (source test like the facts one)

## 5. The message read (L2)

- [ ] 5.1 Migration: rename `need_summaries` → `message_reads`, add items, asks, layer, trigger; keep every row
- [ ] 5.2 `summarize_need` → `read_message`: one schema (summary + `lapses_at` + `due_on` for reply/review/waiting, items, asks), 4,096 tokens, instruction rules from the spec
- [ ] 5.3 Triggers: need opened on a flagged message; `conversations.request_message_read` on open; never for unflagged, phishing, spam, trash, read-only
- [ ] 5.4 Per-item and per-ask vetoes (quote, window, ended, kind, updates/cancels, same_as, amount digits), logged by reason
- [ ] 5.5 `OpenTasks` and `TimeZones` read through the ports; known items of the conversation in the input
- [ ] 5.6 `message_read` v1 published; `need_summarised` kept; Do/Pay due from the first ask with the verb
- [ ] 5.7 `conversations.get_message_read` (404 rules); backfill change for needs that opened before

## 6. schedule module

- [ ] 6.1 Calendars: Campbooks calendar on first use; `capability_granted` → provider calendars (primary syncing); `update_calendar` (sync, colour)
- [ ] 6.2 `apply_calendar_changes`: upserts, removals, series stored once, exceptions; pending member changes kept
- [ ] 6.3 `list_events` with RRULE expansion in the window, `get_event`; 404 for others' calendars
- [ ] 6.4 `add_read_items`: items → Scout events (idempotent on `item_key`, updates/cancels, removed items, kinds off, shared inbox, iCalendar UID link, per-kind "also add to")
- [ ] 6.5 `create_event` / `update_event` (single, one occurrence) / `delete_event` with undo / `respond_event` → `outbound_requested`
- [ ] 6.6 `push_event` ("Add to my calendar") and the outcome handler (etag, 412 kept-theirs notice, failures)
- [ ] 6.7 Member settings (time zone once from the device, working hours, kinds, also-add-to) + `TimeZones` impl
- [ ] 6.8 `free_slots` operation + `Availability` impl
- [ ] 6.9 `event_*` events without PII; `member_left`, `connection_removed`, workspace purge
- [ ] 6.10 `list_events` filters (`kind` repeatable, `q`, `limit`, ≤ 365-day window), RFC 3339 with offset, source link per event; `free_slots` names the calendars counted
- [ ] 6.11 Catalogue metadata: reads `assistant: true`, `mutates: false`; writes `risk: low`; `push_event` confirmation names the target calendar
- [ ] 6.12 `crates/modules/schedule/handbook.md` (`HANDBOOK`, English, ≤ 8 KiB): kinds, Campbooks vs provider calendars, obligations are Due, never writes to a provider without the member
- [ ] 6.13 If campbooks-cloud#305 has merged: Scout tools `my_calendar`, `free_time`, `read_event` (closed arg schemas, ≤ 8 KiB, RecordRefs, never logged) and about entry `schedule.event`; otherwise open the follow-up "wire the Scout tool + about entries after #305 merges"

## 7. Connectors

- [ ] 7.1 Offer `calendar` for Google (two scopes) through the extend path; calendar list on grant; `capability_granted`
- [ ] 7.2 Google Calendar adapter: first read, sync token, 410 resync, normalisation, blobs, budget
- [ ] 7.3 `calendar_changed` v1 published with the cursor in one transaction
- [ ] 7.4 Outbound calendar changes: port confirmation, MAC binding, If-Match, 412 → superseded + re-read, private properties, outcomes, no echo
- [ ] 7.5 Zoho Calendar adapter behind `CAMPBOOKS_CONNECTORS__ZOHO_CALENDAR` (off); 409 `capability_not_offered` while off
- [ ] 7.6 `accessNotConfigured` → calendar stream health reason, mail untouched

## 8. Client

- [ ] 8.1 Calendar place in the sidebar and tab bar, `g s` (`nav.schedule` in the shortcut catalogue), shortcuts sheet
- [ ] 8.2 Agenda, Week (time grid, all-day row, now line, overlap), Month; `1/2/3`, `t`, `j/k`, `n`
- [ ] 8.3 Event sheet: kind chip, From <email> link, quote, Add to my calendar, Edit, RSVP, Remove + Undo, pending and kept-theirs states
- [ ] 8.4 Reader strip: found items, "Scout is reading this…", request read on open, L1-only "This email has a <kind> · Add"
- [ ] 8.5 Settings → Calendar: time zone, working hours, calendars, kinds, also-add-to, Add calendar access
- [ ] 8.6 Kind icons and colour tokens in the design system; Storybook stories for each screen
- [ ] 8.7 i18n en/pt-PT/es-ES/fr, `pnpm test:fit` passes at 375 and 1280

## 9. Evaluation and measurement

- [ ] 9.1 Ask the owner for 30–50 labelled emails (bookings, bills, renewals, newsletters with dates) for `has_dated_thing` / `dated_kind`; agent-written items don't count
- [ ] 9.2 On staging, measure the share of mail flagged, reads per 1,000 emails, and drops by reason; set the `__DATED__FLAG` default from it and record it in the eval report

## 10. Follow-ups to open (campbooks-cloud, label `next`, on project 1)

- [ ] 10.1 Due layer from `tasks.task_changed.v1` (latest revision per task, open with a due date) + `TaskSource` port, drawn as the calendar-ui "What is due" rows (Pay/Do, amount only when `has_amount`, Due 17:00, Overdue in danger colour, done drops off, held-on-due-day shows only Held, Week's Due lane above All day)
- [ ] 10.2 `TimeHeld` / `TimeReleased` → hold events; hold `event_changed` / `event_cancelled` back to tasks
- [ ] 10.3 Microsoft Graph calendar, CalDAV, Google push channels, series edits, calendar sharing

## 11. Wrap-up

- [ ] 11.1 OpenAPI regenerated (`cargo xtask openapi`) and client types regenerated (`pnpm --dir clients api:generate`), committed from CI's output if needed
- [ ] 11.2 Changelog entry files in `changelog/unreleased/<kind>/`
- [ ] 11.3 Gates green in CI (AGENTS.md → Code quality)
- [ ] 11.4 UI verified at 375px and desktop, recording in the PR (`## Recording`)
- [ ] 11.5 `openspec archive add-schedule-module --yes` before merge; baseline specs reflect the change
