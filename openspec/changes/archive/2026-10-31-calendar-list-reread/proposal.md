# Proposal

## Why

Connectors read a connection's calendar list only when calendar access is
granted. A calendar shared with the member later, or one they create at Google
or Zoho, never reaches Campbooks until they reconnect calendar access. The same
goes for a calendar whose ownership the provider had not told us yet
(`owned` unknown on calendars made before calendar roles).

Tracking: https://github.com/notacamp/campbooks-cloud/issues/405

## What Changes

- **Connectors:** for each connection with the `calendar` capability, the
  calendar stream reads the provider's calendar list again about once a day
  (jittered), in the same run that syncs events. Nothing is read while the
  connection has no `calendar` capability. A provider error on this read never
  stops event sync.
- **Publish:** the list is published in the same `calendar-list` v1 blob as at
  grant time, through a new event, `connectors.calendar_list_read` v1. So
  `capability_granted` keeps meaning a grant. The event carries ids and a blob
  reference only.
- **Schedule:** it applies the event with the repeated-grant rule. A calendar
  it does not have is added **off**, following when the account does not own it
  and mine otherwise. `owned` is filled where unknown. Nothing else changes.
  A calendar found later is never turned on by itself.
- Handbook: new calendars shared later show up in Settings → Calendar, off,
  within a day.
- A migration adds `connectors.streams.calendar_list_due_at`.

Modules touched: `connectors`, `schedule`.

## Non-goals

- Calendars removed at the provider. Their events already stop through sync
  (a 404 or 410 on the calendar); this can be handled separately.
- Any screen change: the calendars appear in the existing Settings → Calendar.
