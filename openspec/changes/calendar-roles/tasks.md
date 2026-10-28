# Tasks

## 1. Connectors say whose calendar it is

- [ ] 1.1 `contracts/calendar-list.v1.schema.json`: optional `owned` boolean per calendar
- [ ] 1.2 `ListedCalendar.owned: Option<bool>`; Google from `accessRole` (`owner` → true, other values → false, absent → None); Zoho from `privilege` (`owner` → true, other values → false, absent → None); written into the blob; unit tests for both

## 2. Schedule stores the role

- [ ] 2.1 Migration: `schedule.calendars.following boolean NOT NULL DEFAULT false`, `owned boolean NULL`, `CHECK (source = 'provider' OR NOT following)`; backfill `following = true` for provider calendars with `writable = false`
- [ ] 2.2 `grant_calendars`: new calendars get `owned` and `following = (owned = false)`; a repeated grant sets `owned` where it is NULL, nothing else
- [ ] 2.3 `CalendarRow`/`CalendarView`: `role` (`mine` | `following` | `off`) and `owned`; OpenAPI and generated client types updated
- [ ] 2.4 `PATCH …/calendars/{id}` accepts `role`; refuses a role and `syncing` that disagree, and any role but mine on the Campbooks calendar (`invalid_calendar`); Off → on publishes `calendar_sync_changed`, Mine ↔ Following does not; `announce_calendar` on a role change; tracked `CalendarRoleChanged`
- [ ] 2.5 Free slots count only the Campbooks calendar and Mine calendars (`domain/free.rs`)
- [ ] 2.6 Holds refuse a Following calendar as `calendar_followed` (server messages in en, pt, es, fr)
- [ ] 2.7 `invitations::standing` answers `Hidden` for a Following calendar
- [ ] 2.8 Integration tests: grant defaults, repeated grant, PATCH role transitions and refusals, free slots skip a Following calendar, hold refused, invitation closed and re-announced on a role change
- [ ] 2.9 `crates/modules/schedule/handbook.md`: Mine / Following / Off, free time counts only Mine

## 3. Before merge

- [ ] 3.1 Changelog entry files (`changelog/unreleased/added/399-calendar-roles.md`, `changelog/unreleased/fixed/399-followed-invitations.md`)
- [ ] 3.2 `openspec validate calendar-roles --strict`; CI green
- [ ] 3.3 Terminal recording (vhs) of the API: a calendar's role, a PATCH to following, free slots without it; then archive this change
