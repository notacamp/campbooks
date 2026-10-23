# Calendar sync: Google and Zoho calendars, two ways

Modules: `connectors` (Google Calendar and Zoho Calendar adapters, the
`calendar` capability offered), `schedule` (provider calendars, their events
applied, the member's changes written back, "Add to my calendar"), and the
`schedule` client module (the parts of the Calendar screens that need a
provider calendar). Tracking:
https://github.com/notacamp/campbooks-cloud/issues/42. This is the second
half of the calendar work; the first half is `add-schedule-module`, which
ships the Campbooks calendar, the dates Scout reads from email and the
Calendar screens. Read its `design.md` (→ *Two-way sync*, *Data ownership
and events*) first: the design of sync lives there.

## Why

`add-schedule-module` gives every member a Campbooks calendar, but it cannot
show the calendar they already keep in Google or Zoho, and nothing Scout
finds can reach it. The owner chose to ship in two changes so the server,
the reading of dates from email and the screens could go first.

## What Changes

- **connectors** offers the `calendar` capability for Google (and Zoho behind
  a flag) through the existing extend path, reads calendars incrementally and
  publishes `connectors.calendar_changed`.
- **schedule** creates provider calendars from the grant, applies their
  changes, writes the member's changes back (create, edit, delete, RSVP) with
  etag checks, and copies a Campbooks event to a provider calendar on
  request ("Add to my calendar") or by a per-kind rule.
- The Calendar screens show provider events, invitations, "changed in Google
  meanwhile; kept theirs", "Add to my calendar", "Also add to" and "Add
  calendar access".
- Code that already exists in `add-schedule-module` and is inert until this
  lands (the `schedule` side of `outbound_requested`, `push_event`,
  `unpush_event`, `calendar_sync_changed`, the calendar-change MAC) is
  specified here, because nothing can exercise it before.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `connectors`: the `calendar` capability, adapters, change stream, outbound
  changes (ADDED requirements).
- `schedule`: provider calendars and events (ADDED requirements).
- `calendar-ui`: provider events in the sheet, calendar access in Settings
  (ADDED requirements).

## Impact

- **Members reconnect once** to add calendar access (Google
  `calendar.events` + `calendar.calendarlist.readonly`; Zoho
  `ZohoCalendar.calendar.READ`, `ZohoCalendar.event.ALL`). Google's calendar
  scopes are sensitive, not restricted: they join the verification tracked in
  campbooks-cloud#53.
- Self-hosters enable the Google Calendar API on their Google project and
  add the scopes to their Zoho app. Without either, the Campbooks calendar
  still works.

## Non-goals

Microsoft 365 / Graph and CalDAV, Google push channels, editing a whole
series, sharing calendars, typing provider events with Jev: as in
`add-schedule-module`.
