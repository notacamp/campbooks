# Tasks

## 1. schedule
- [x] 1.1 Migration `20261108010000_schedule__invitations.sql`: organiser columns, `invitation_revision`, `invitation_open`, `invitation_backfills` (maintenance read policy) and its seed
- [x] 1.2 The batch's organiser kept (`organizer_person` from the facts strategy's address test); `EventView.organizer`
- [x] 1.3 `domain::invitations`: `standing`, `announce`, `announce_calendar`, `close_all`; `schedule.invitation_changed` v1 (event, contract, README)
- [x] 1.4 Announced from the publish helpers, a batch changing only the answer or organiser, `respond_event`, a calendar turned on or off, a removed connection and a member who left
- [x] 1.5 `respond_event`: the answer's undo window, `schedule.release_response`
- [x] 1.6 Backfill: `schedule.dispatch_invitation_backfills` + `schedule.announce_invitations`; `architecture.toml` maintenance entry; purge
- [x] 1.7 `schedule.member` visibility port, registered by the server
- [x] 1.8 Tests: contract, announcing (open, moved, cancelled, organiser, answered, started, service), the undo window, hidden and removed, the backfill, the port

## 2. attention
- [x] 2.1 `attention.project_invitation`: Decide item, order-proof, tombstone, lapse
- [x] 2.2 Bands: `event_today`, `event_tomorrow`, `event_on`; never overdue, no `waiting_answer`
- [x] 2.3 Tests: projection (open, answered, reopened, cancelled, stale, started, only the member), bands (today, tomorrow, 5 and 12 days)
- [x] 2.4 Handbooks (attention, schedule)

## 3. assistant
- [x] 3.1 `what_needs_me` names an invitation (its event's line, who asked); test

## 4. Client
- [x] 4.1 schedule: `useInvitations` (rows in words, free/busy), `useInvitationAnswers` (toast, Undo for the window); `useHomeDay` hides invitations; unit tests
- [x] 4.2 attention: the Decide row, keys 1/2/3, Snooze, Enter opens Calendar, read-only calendar; reason sentences
- [x] 4.3 i18n in `en`, `es-ES`, `fr`, `pt-PT`
- [x] 4.4 `WhatNeedsYou` invitation tests; stories (free, busy); Home demo data; recording scenario

## 5. Finish
- [x] 5.1 OpenAPI regenerated (`openapi/openapi.json`: `BandRule`, `EventView`, `OrganizerView`; `openapi/events.json`: `schedule.invitation_changed`)
- [x] 5.2 Client types regenerated (`schema.gen.ts`)
- [x] 5.3 Tracking plan: patch `0013-home-invitations.yaml` (Coverage rows), generated
- [x] 5.4 Changelog entry `changelog/unreleased/added/home-invitations.md`
- [x] 5.5 Gates green in CI
- [x] 5.6 UI verified at 375px and desktop (recording)
