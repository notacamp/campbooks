# Tasks

Moved here from `add-schedule-module` (its §7) plus the follow-ups that wait
for sync. Builders don't compile or test locally: push and read CI.

## 1. Connectors (was add-schedule-module §7)

- [x] 1.1 Offer `calendar` for Google (two scopes) through the extend path; calendar list on grant; `capability_granted`
- [x] 1.2 Google Calendar adapter: first read, sync token, 410 resync, normalisation, blobs, budget
- [x] 1.3 `calendar_changed` v1 published with the cursor in one transaction
- [x] 1.4 Outbound calendar changes: port confirmation, MAC binding, If-Match, 412 → superseded + re-read, private properties, outcomes, no echo
- [x] 1.5 Zoho Calendar adapter behind `CAMPBOOKS_CONNECTORS__ZOHO_CALENDAR` (off); 409 `capability_not_offered` while off
- [x] 1.6 `accessNotConfigured` → calendar stream health reason, mail untouched

## 2. Connectors consume what schedule publishes

- [x] 2.1 Consume `schedule.calendar_sync_changed` (read a calendar again from the start) and `schedule.outbound_requested`, and actually publish `calendar_changed` / `calendar_change_completed`; remove the matching unconsumed entries from `architecture.toml`
- [x] 2.2 `RetagBindings` for `change_requests` (the binding rekey when a connection's ids change)
- [x] 2.3 Detach the blobs of calendar batches and calendar lists when their rows go (connectors' owner decides the retention)

## 3. Scout chat

- [x] 3.1 Scout chat proposal rows for the schedule writes (create, update, delete, respond, push, unpush): `crates/modules/assistant/src/features/chat/propose.rs`; client `modules/assistant/api/proposals.ts` (mark them RUNNABLE) and `ProposalCards.tsx`, with i18n in the four languages

## 4. Missing server fields

- [x] 4.1 `ReadItem.event_id` (the strip links an item to the event it became)
- [x] 4.2 The provider name on `CalendarView` (the sheet says "Google", not "the provider")
- [x] 4.3 `NewEvent.source` (the source link when a client creates an event from an email)

## 5. Client

- [x] 5.1 Show "Add to my calendar" only once a provider calendar that can be written exists, and show "Add calendar access" in Settings → Calendar once sync lands (hidden in the first change, because its consent cannot succeed before connectors offers `calendar`; the code is behind `CALENDAR_ACCESS_OFFERED` in `CalendarSettings.tsx`)
- [x] 5.2 Provider events, invitations and the "kept theirs" notice verified in the real app; i18n and `pnpm test:fit` at 375 and 1280

## 6. Wrap-up

- [x] 6.1 OpenAPI and client types regenerated from CI's output
- [x] 6.2 Changelog entry files in `changelog/unreleased/<kind>/`
- [x] 6.3 Gates green in CI; recording in the PR (`## Recording`)
- [x] 6.4 `openspec archive add-calendar-sync --yes` before merge; baseline specs reflect the change

## Follow-ups

- A restoring undo (update, respond) writes back the values read when the card was proposed; events have no revision, so a provider change between the card and Confirm is overwritten by Undo. Taking the undo from the answer's prior values (`UpdatedEvent`, `previous_response`) is exact but needs a `from_response` rename.
- After a reload, the undo of a create, delete, push or unpush needs the change's answer, which only lives in component state: a settled card shows no Undo, as task cards do.
