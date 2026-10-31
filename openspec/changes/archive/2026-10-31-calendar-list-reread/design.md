# Design

## A new event, not `capability_granted`

Alternatives:

1. **Publish `capability_granted` again every day.** Schedule already handles a
   repeated grant, so this is the smallest diff. But the event means "a member
   gave consent". Other consumers (conversations, tracking, audit) would see a
   daily grant that never happened.
2. **A new event, `connectors.calendar_list_read` v1 (chosen).** Same
   `calendar-list` blob, so no new document. Schedule subscribes and applies it
   with the same function the grant uses. The event carries the workspace, the
   connection, the member whose connection it is (`granted_by`, an id) and the
   blob reference. No provider calendar id (it can be an address) appears in
   it; it lives only in the blob.

## The same rule, one function

`grant_calendars` and the new handler share `apply_list`. A calendar the
module has not got is inserted; one it has gets `owned` filled when NULL and
nothing else. The only difference is `syncing`: a grant turns the primary
calendar on, a later read never turns anything on (`syncing = false`).
`following = owned IS FALSE` in both. `is_primary` is the provider's word in
both.

## A late read makes nothing

Events are delivered in any order, so a `calendar_list_read` can be handled
after `connection_removed` or after the member left, both of which delete the
connection's calendars. A re-read therefore adds calendars only while the module
still holds at least one provider calendar for that workspace, connection and
member (any role counts, Off too). Only a grant makes a connection's first calendar.

A bare check is not enough: a delete already running cannot see a row
inserted after its snapshot, so that row would survive it. So a transaction-
scoped advisory lock serialises them. The key is
`hashtextextended('schedule.calendar_list:' || connection_id, 0)`: namespaced
so it cannot meet another module's advisory locks, and without the workspace
because a connection id is unique. One helper, `domain::locks::connection`,
derives it. The re-read and `connection_removed` each take the lock of their
one connection, first in their transaction. Forgetting a member is per member,
so it takes the lock of each of the member's provider connections, in
ascending connection id order, before deleting anything; the same order keeps
two forgets, or a forget and a re-read, from waiting on each other in a circle.
The guard is checked after the lock is held.

## Cadence and the marker

The list is read when it is due, at the start of a calendar stream's run that
already has calendars (a run with none lists them anyway, as today).

Alternatives for where "when" lives:

1. **A second schedule row / a second job.** More machinery for one call a day.
2. **A column on the stream (chosen):** `connectors.streams.calendar_list_due_at`.
   The run compares it to now. After a read it is set to now + 24 hours + up to
   2 hours of jitter, taken from the stream's id and the time so the streams of
   one instance do not line up. After a provider error it is set to now + 1
   hour, so a failing provider is asked every hour, not every minute. When
   the calendar budget has no room the column is left alone and the next run
   asks again.
3. Using `last_full_at` of the calendars: it says nothing about the list.

The migration spreads the existing calendar streams over the next 24 hours so
a deploy does not read every list at once. A NULL is treated as due now.

## Trouble stays small

The read is made before the event sync, takes one request from the connection's
calendar budget, and its failure is logged and only moves the marker. It does
not touch `health_reason`, `consecutive_failures` or the connection's health:
event sync reports those. A refused token or an API that is not enabled will be
met, and reported, by the sync that follows in the same run.

## Connectors' own calendar rows

New calendars are remembered in `connectors.calendars` with `syncing = false`
(the grant remembers the primary as syncing). Turning one on later arrives as
`calendar_sync_changed`, as for any calendar.

## Tracking

No new tracked event. The calendars added are tracked as `CalendarAdded` by
the shared function, as a grant does.
