# Calendar surface — `/api/app`

**Status:** 🟡 partial — `/api/v1` has raw event CRUD + RSVP only · **Priority:** P2 · **Depends on:** [`00-auth.md`](00-auth.md)

## What the SPA must render

- **Calendar toolbar**: view switcher (agenda / day / week / month), date navigation (prev / next / today), account sidebar toggle.
- **View grids** (patterns from `Campbooks::Calendar::{AgendaList,DayGrid,WeekGrid,MonthGrid}`):
  - *Agenda*: upcoming items grouped by day — events, pending reminders, snoozed threads (`:email_scheduling` gated), scheduled emails (`:email_scheduling` gated).
  - *Day*: hour-row time grid, overlapping events in columns, now-line.
  - *Week*: 7-column time grid with per-day overlap columns.
  - *Month*: 5-week tile grid with event chips.
- **Event detail sheet**: title / time / location / attendees / RSVP buttons (needs\_action → accepted / declined / tentative) / color swatch / description / linked source email.
- **Event CRUD form**: create, edit, delete with recurrence scope (single / all / future). Drag-to-reschedule from day and week grids.
- **Event color picker** (`Campbooks::ColorSwatchPicker`): uses the fixed Google event palette (`Calendars::EventColors` maps hex ↔ Google `colorId` 1–11); `CalendarEvent#display_color` falls back to the calendar color.
- **Event types manager** (`/event_types`): list, create, edit, delete, one-click starter set (empty-state shortcut).
- **Account sidebar**: all readable calendar accounts with their calendars; per-calendar sync toggle, color override, show/hide visibility; account-level sharing panel; refresh (re-pull provider calendar list); disconnect.
- **ICS import**: file upload → confirm → import into a chosen writable calendar.

## Web routes being replaced

| Web route | Controller#action | Notes |
|---|---|---|
| `GET /calendar` | `calendar#index` | `?view=agenda\|day\|week\|month&date=YYYY-MM-DD`; loads `Calendars::PageData` |
| `GET /calendar_events/:id` | `calendar_events#show` | Event detail |
| `GET /calendar_events/new` | `calendar_events#new` | Create form |
| `POST /calendar_events` | `calendar_events#create` | Create + enqueue `Calendars::EventWriteJob` |
| `GET /calendar_events/:id/edit` | `calendar_events#edit` | Edit form |
| `PATCH /calendar_events/:id` | `calendar_events#update` | Update + enqueue write-through |
| `DELETE /calendar_events/:id` | `calendar_events#destroy` | Async delete (202, `outbound_pending`) |
| `POST /calendar_events/:id/rsvp` | `calendar_events#rsvp` | RSVP status update |
| `PATCH /calendar_events/:id/reschedule` | `calendar_events#reschedule` | Drag-to-reschedule from day/week grids |
| `GET /event_types` | `event_types#index` | Event types list |
| `POST /event_types` | `event_types#create` | Create event type |
| `PATCH /event_types/:id` | `event_types#update` | Edit event type |
| `DELETE /event_types/:id` | `event_types#destroy` | Delete event type |
| `POST /event_types/starters` | `event_types#starters` | Seed starter set (`EventType::STARTERS`) |
| `PATCH /calendar_accounts/:id` | `calendar_accounts#update` | Account settings (name, permissions) |
| `DELETE /calendar_accounts/:id` | `calendar_accounts#destroy` | Disconnect account |
| `GET /calendar_accounts/:id/sharing` | `calendar_accounts#sharing` | Sharing panel (viewer/editor/manager roles) |
| `POST /calendar_accounts/refresh` | `calendar_accounts#refresh` | Re-pull provider calendar list on demand |
| `PATCH /calendar_accounts/:id/calendars/:calendar_id` | nested `calendars#update` | Per-calendar sync toggle / color override |
| `PATCH /calendar_visibilities/:id` | `calendar_visibilities#update` | Show/hide a calendar on `/calendar` |
| `GET /calendar_import/new` | `calendar_import#new` | ICS import form |
| `POST /calendar_import` | `calendar_import#create` | Process ICS upload via `Calendars::IcsImporter` |

## `/api/v1` coverage today

🟡 **Partial — raw event CRUD only.** `GET /api/v1/calendar_events` (index, `?start_after`/`?start_before`/`?calendar_id` filters), `GET /api/v1/calendar_events/:id` (show), `POST /api/v1/calendar_events` (create), `PUT /api/v1/calendar_events/:id` (update), `DELETE /api/v1/calendar_events/:id` (async, 202, `outbound_pending`), `POST /api/v1/calendar_events/:id/rsvp` — scoped `calendar:read`/`calendar:write`. `Api::V1::CalendarEventSerializer` exists and is reusable.

**Not covered:** screen-shaped `Calendars::PageData` aggregation (multi-collection view with accounts sidebar), `reschedule`, calendar account management, per-calendar sync/color, calendar visibility, event types, ICS import — all net-new.

`GET /api/v1/reminders` exists (raw) but is the Hotwire reminder model, not the calendar page view.

## `/api/app` endpoints to build

| Method + path | Purpose | Reuses |
|---|---|---|
| `GET  /api/app/calendar` | Screen payload: accounts sidebar + events / reminders / snoozed / scheduled collections for the requested `?view=&date=` | `Calendars::PageData.for(user:, view:, date:, entitlements:)`; `Api::V1::CalendarEventSerializer` |
| `GET  /api/app/calendar_events/:id` | Event detail (`detail: true`) | `Api::V1::CalendarEventSerializer` |
| `POST /api/app/calendar_events` | Create event; enqueue write-through | `Calendars::EventWriteJob`; `CalendarEventsController#create` logic (verify writable calendar gate) |
| `PATCH /api/app/calendar_events/:id` | Update event; enqueue write-through | `Calendars::EventWriteJob`; `recurrence_scope` param (single/all/future) |
| `DELETE /api/app/calendar_events/:id` | Async delete → 202 | `Calendars::EventWriteJob`; same `outbound_pending` pattern |
| `POST /api/app/calendar_events/:id/rsvp` | RSVP update | `Calendars::EventWriteJob`; same as v1 |
| `PATCH /api/app/calendar_events/:id/reschedule` | Drag-to-reschedule (new `start_at`/`end_at`) | `CalendarEventsController#reschedule` logic; `Calendars::EventWriteJob` |
| `GET  /api/app/event_types` | Workspace event types list | `EventType`; `EventTypesController#index` logic |
| `POST /api/app/event_types` | Create event type | `EventTypesController#create` logic |
| `PATCH /api/app/event_types/:id` | Update event type | `EventTypesController#update` logic |
| `DELETE /api/app/event_types/:id` | Delete event type | `EventTypesController#destroy` logic |
| `POST /api/app/event_types/starters` | Seed starter set | `EventType::STARTERS`; `EventTypesController#starters` logic |
| `PATCH /api/app/calendar_accounts/:id` | Update account settings | `CalendarAccountsController#update` / `#update_account_settings` logic |
| `DELETE /api/app/calendar_accounts/:id` | Disconnect account | `CalendarAccountsController#destroy` logic |
| `GET  /api/app/calendar_accounts/:id/sharing` | Sharing panel (roles, members) | `CalendarAccountsController#sharing`; `CalendarAccountUser::ROLES` |
| `POST /api/app/calendar_accounts/refresh` | Re-pull provider calendar list | `CalendarAccountsController#refresh` logic |
| `PATCH /api/app/calendar_accounts/:id/calendars/:calendar_id` | Per-calendar sync toggle / color | Nested `calendars#update`; `CalendarAccountsController` |
| `PATCH /api/app/calendar_visibilities/:id` | Show/hide a calendar | `CalendarVisibilitiesController#update` logic |
| `POST /api/app/calendar_import` | Import ICS → `{ imported:, skipped:, errors: }` | `Calendars::IcsImporter` (verify `.new(...).import` vs `.call` interface) |

## Read models / serializers needed

- **`Api::V1::CalendarEventSerializer`** — reuse; `detail: true` for full payload (attendees, recurrence, color, rsvp_status, source_email_message).
- **`Api::App::CalendarPageSerializer`** (new) — screen-shaped payload from `Calendars::PageData::Result`: `{ view, date, range: { start, end }, prev_date, next_date, calendar_accounts: [...], has_calendars: bool, events: [...], reminders: [...], snoozed_threads: [...], scheduled_emails: [...] }`. `snoozed_threads` and `scheduled_emails` are only present when the workspace has the `:email_scheduling` entitlement.
- **`Api::App::CalendarAccountSerializer`** (new) — account attributes + nested `calendars` array with `{ id, name, color, syncing, is_primary, visibility }`.
- **`Api::App::EventTypeSerializer`** (new) — event type CRUD payload.
- **`Api::App::CalendarImportResultSerializer`** (new) — `{ imported: N, skipped: N, errors: [] }`.

**Color contract:** `CalendarEvent#display_color` returns hex. The SPA sends hex on create/update; the server maps to provider `colorId` via `Calendars::EventColors` (hex ↔ Google colorId 1–11) in `EventWriter`. The SPA should render `display_color` from the serializer, never its own mapping.

## Real-time

Calendar is **poll-driven** — no `broadcasts_to` on `CalendarEvent` today, and `Calendars::PageData` is a pure loader with no side effects. The SPA should re-fetch `GET /api/app/calendar` on window focus or after any mutation (the existing approach in Hotwire was full page reload on redirect).

An optional `calendar_<user_id>` JSON channel could notify the SPA when a sync cycle completes (`CalendarScanJob` finishes), avoiding manual polling. This is additive — not required for the initial React surface. See [`realtime.md`](realtime.md).

`CalendarWebhooksController#google_receive` (public, token-verified) already enqueues incremental syncs on provider push — the SPA benefits from this automatically through the poll.

## Open questions

- **`reschedule` params:** verify the exact params `CalendarEventsController#reschedule` accepts (`start_at`/`end_at` absolute vs duration delta) before implementing the SPA drag handler.
- **`Calendars::IcsImporter` interface:** confirm public API — `.new(calendar:, file:).call` or `IcsImporter.import(...)` — before wiring the endpoint.
- **Recurrence scope param name:** verify `recurrence_scope` values and the param key in `CalendarEventsController#update` / `#destroy` (single/all/future — confirm exact strings in the controller).
- **`snoozed_threads` / `scheduled_emails` entitlement gate:** `Calendars::PageData` gates these behind `:email_scheduling`. The SPA should read the entitlement from the `me` bootstrap payload and suppress those UI sections when absent, rather than relying on absent keys in the calendar response.
- **Writable calendar validation on create:** the v1 controller checks `Calendar.where(calendar_account: Current.user.writable_calendar_accounts, is_writable: true, syncing: true)`. Extract this into a service or concern so the `/api/app` controller doesn't duplicate it.
- **`CalendarAccountsController#update` split:** the controller has `#update_account_settings` and `#update_user_permissions` private methods — verify whether these map to one or two `/api/app` endpoints.
