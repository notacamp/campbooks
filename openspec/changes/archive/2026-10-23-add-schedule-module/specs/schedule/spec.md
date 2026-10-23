## Purpose

The `schedule` module keeps each member's Campbooks calendar and its events,
which hold the things Scout read from email. It owns event kinds, recurrence,
the member's time zone and working hours, and free time. Provider calendars
(Google, Zoho) and writing changes back to them arrive with `add-calendar-sync`.

## ADDED Requirements

### Requirement: Schedule scopes
The `schedule` module SHALL declare the scopes `schedule:read` and
`schedule:write`. Listing and reading events, calendars, settings and free
slots SHALL require `schedule:read`; creating, changing, deleting, answering
and pushing events and changing calendars and settings SHALL require
`schedule:write`.

#### Scenario: Anonymous caller
- **WHEN** a client calls `GET /api/v1/schedule/events` without an access token
- **THEN** the response is 401 as `application/problem+json`

#### Scenario: Token without the scope
- **WHEN** a client with only `schedule:read` calls `POST /api/v1/schedule/events`
- **THEN** the response is 403 as `application/problem+json` naming `schedule:write`

### Requirement: Calendars are their member's
Every calendar SHALL belong to one member of one workspace, and only that
member SHALL list it, read its events or change it; for anyone else it SHALL
not exist (404). Each member SHALL have exactly one Campbooks calendar,
created the first time anything needs it, which no provider sees.

#### Scenario: The first event
- **WHEN** Inês's first event is added and she has no calendar yet
- **THEN** she has one Campbooks calendar, and it holds the event

#### Scenario: Someone else's calendar
- **WHEN** Rui asks for one of Inês's calendars or events by id
- **THEN** the answer is 404

### Requirement: Event kinds
Every event SHALL have one kind from a fixed set: `charge`, `appointment`,
`meeting`, `travel`, `stay`, `show`, `reservation`, `delivery`, `expiry`,
`hold` or `none`. A member SHALL be able to change an event's kind.
Obligations (pay by, submit by, renew by) SHALL never be events: they are
asks owned by `tasks`.

#### Scenario: The member sets a kind
- **WHEN** Inês sets "Dentist" from `none` to `appointment`
- **THEN** the event is `appointment` and `schedule.event_changed` is published

### Requirement: Events from what Scout read
The module SHALL consume `conversations.message_read` v1 and, for each of
its `items`, create an event on the Campbooks calendar of the inbox's
personal owner, with origin `scout`, the item's kind and times, `found_by`
from the read's layer, and a source link to the message. It SHALL read the
item's title and place from the `texts` document the read's event points at
(a blob the message holds, which no event or job payload repeats). The
event SHALL be keyed by the item's `item_key`, so a repeated delivery
changes nothing. An item that `updates` an earlier item SHALL change that
item's event; an item that `cancels` one SHALL cancel it. The module SHALL
add nothing when the read names no personal owner (a shared inbox), when the
member turned that kind off, when the member removed the event for that
item before, or when the read's `asks` are all it carries.

#### Scenario: A flight confirmation
- **WHEN** a read of Inês's flight confirmation carries a `travel` item "Flight LIS → AMS" on Friday 6 November from 07:40 to 11:55 Lisbon time
- **THEN** Inês's Campbooks calendar has that event, origin `scout`, linking the email, and `schedule.event_created` is published

#### Scenario: Delivered twice
- **WHEN** the same `message_read` event is delivered again
- **THEN** nothing is added or changed

#### Scenario: Moved by a later email
- **WHEN** a later email in the same conversation is read with an item that `updates` the flight to 08:10
- **THEN** the same event now starts at 08:10 and `schedule.event_changed` is published

#### Scenario: Cancelled by a later email
- **WHEN** the hotel's cancellation is read with an item that `cancels` the stay
- **THEN** the stay is cancelled and `schedule.event_cancelled` is published

#### Scenario: Removed before
- **WHEN** Inês removed Scout's "Parcel from Lojas Sol" event and the message is read again
- **THEN** no event is added

#### Scenario: A kind turned off
- **WHEN** Inês turned off `delivery` and a delivery is read
- **THEN** no event is added

#### Scenario: A shared inbox
- **WHEN** a booking arrives in the shared inbox geral@costaobras.example
- **THEN** no event is added to anyone's calendar

### Requirement: Member settings
Each member SHALL have settings: a time zone (an IANA name), working hours
per weekday, the kinds Scout adds (all by default) and, per kind, an "also
add to" calendar (none by default). The client SHALL send the device's time
zone once (`device_time_zone`), which the module takes only when the member
has none and ignores after; from then on only the member changes it
(`time_zone`).
Until a member has a time zone, UTC SHALL be used. The module SHALL implement
the kernel port `TimeZones`, which tells another module a member's time
zone, and SHALL refuse a time zone that is not an IANA name and working
hours that end before they start.

#### Scenario: First open on a phone in Lisbon
- **WHEN** Inês has no time zone and her app reports `Europe/Lisbon`
- **THEN** her time zone is `Europe/Lisbon`, and a later report of `Europe/Madrid` from a laptop changes nothing

#### Scenario: Not a zone
- **WHEN** a member sets their time zone to `GMT+1`
- **THEN** the answer is 400 and the setting is unchanged

### Requirement: Free time
The module SHALL answer, for a member, a window and a duration, the free
slots within the member's working hours, in their time zone, that no event
of a syncing calendar or of the Campbooks calendar marked busy overlaps
(cancelled events and all-day events do not count). The same answer SHALL be
available to other modules through the kernel port
`Availability::free_slots(member, window, duration)` and to the member as
`schedule.free_slots`, and the answer SHALL name the calendars it counted.

#### Scenario: A free hour on Tuesday
- **WHEN** Inês works 09:00–18:00 and on Tuesday has events 09:00–12:30 and 14:00–18:00, and asks for an hour that day
- **THEN** the answer is 12:30–14:00, naming her Campbooks calendar as the calendar counted

#### Scenario: A weekend
- **WHEN** Inês has no working hours on Saturday and asks for an hour on Saturday
- **THEN** the answer is empty

### Requirement: Listing events
`schedule.list_events` SHALL return the caller's events and expanded
occurrences that overlap a window of at most 365 days, from the calendars
they sync and their Campbooks calendar, optionally only of one or more kinds
(`kind`, a comma-separated list), only those whose title contains a text (`q`, ignoring
case and accents) and at most `limit` of them, filtered on the server so a
caller never has to page through the rest. Times SHALL be RFC 3339 with the
offset of the member's time zone, and each event SHALL carry its kind,
calendar and source link. A window longer than 365 days SHALL be refused
with 400.

#### Scenario: Trips this year
- **WHEN** Inês lists `kind=travel&kind=stay` from 1 November for 365 days
- **THEN** she gets her flights and hotel stays in that window, each with its email link, and nothing else

#### Scenario: Looking for the dentist
- **WHEN** Inês lists the next 90 days with `q=dentista` and `limit=1`
- **THEN** she gets the first event whose title contains "Dentista"

#### Scenario: Too long a window
- **WHEN** a caller asks for two years at once
- **THEN** the answer is 400

### Requirement: Scout can read the calendar
The read operations (`schedule.list_events`, `schedule.get_event`,
`schedule.free_slots`, `schedule.list_calendars`) SHALL be in the operation
catalogue with `assistant: true` and `mutates: false`. The write operations
that change events (`create_event`, `update_event`, `delete_event`,
`restore_event`, `respond_event`) SHALL be
`assistant: true`, `mutates: true`, `risk: low`, scope `schedule:write`, and
`update_calendar` and `update_settings` SHALL be `assistant: false`.
Scout SHALL have the tools `my_calendar` (list
events), `free_time` (free slots) and `read_event` (one event), each with an
argument schema that allows no other property and an answer of at most 8 KiB,
records returned as references and never logged, and the about entry
`schedule.event` that reads the event (`schedule.get_event`) and, when it was
read from an email the member can still read, what Scout read there
(`conversations.get_message_read`). The module SHALL ship a handbook
(`crates/modules/schedule/handbook.md`, `HANDBOOK`), in English and at most
8 KiB, that explains event kinds, the Campbooks calendar beside provider
calendars, that obligations are tasks shown as Due, and that Scout never
writes to a provider without the member.

#### Scenario: What's on Friday
- **WHEN** a member asks Scout what is on Friday
- **THEN** Scout calls `my_calendar` for Friday and answers from at most 8 KiB of events, without asking to confirm

#### Scenario: An event from an email the member cannot read
- **WHEN** a chat is about an event whose source email is in an inbox the member no longer reads
- **THEN** Scout is given the event and nothing of the email

### Requirement: Writes answer with what undoes them
Each write that changes events SHALL declare its undo in `x-campbooks.undo`:
`create_event` ↔ `delete_event`, `delete_event` ↔ `restore_event`,
`update_event` ↔ `update_event`, `respond_event` ↔ `respond_event`. Each answer SHALL carry, at its top level,
what the inverse call needs: `delete_event` the `undo_token` that
`restore_event` takes as `{undo_token}`; `update_event` the `id` of the event
changed and, named exactly like the fields of its request, the previous
values of the fields it changed (`title`, `kind`, `starts`, `ends`,
`time_zone`, `place`, `notes`, `busy`; empty text for none), so that sending
them back as the body of the same operation undoes the change;
`respond_event` the answer before as `previous_response`.

#### Scenario: Undo a move
- **WHEN** Inês moves "Dentist" from 15:00 to 16:00 and sends the `starts` and `ends` of the answer back as a `PATCH` to the same event
- **THEN** the event is at 15:00 again

#### Scenario: Undo a delete
- **WHEN** Inês deletes "Dinner at Taberna" and sends the answer's `undo_token` to `restore_event`
- **THEN** the event is back, and `restore_event`'s own undo is `delete_event`

### Requirement: Changing events
A member SHALL be able to create an event on their Campbooks calendar, change
a single event or one occurrence of a series (never the whole series), and
delete an event with an undo. A change to an event on the Campbooks calendar
SHALL never be sent to a provider. A delete SHALL hide the event at once and
keep it for the undo window (10 seconds); `restore_event`, with the delete's
`undo_token`, SHALL bring it back. Changing the whole of a series SHALL be
refused (`series_not_editable`); an occurrence is changed by naming it.

#### Scenario: Moving the dentist
- **WHEN** Inês moves "Dentist" on her Campbooks calendar from 15:00 to 16:00
- **THEN** the event shows 16:00 and `schedule.event_changed` is published

#### Scenario: Deleted and undone
- **WHEN** Inês deletes "Dinner at Taberna" and presses Undo
- **THEN** the event is back as it was

#### Scenario: A whole series
- **WHEN** a caller changes a recurring event without naming an occurrence
- **THEN** the answer is 400 `series_not_editable`

### Requirement: Published events carry no personal data
The module SHALL publish `schedule.event_created`, `schedule.event_changed`
and `schedule.event_cancelled` (v1) in the transaction of the change, with
the event's id, calendar, owner, kind, start, end, whether it is all-day,
origin and source link, and no title, place, notes or attendees. A change
applied from a provider batch and a change by the member SHALL publish the
same events. Expanding occurrences SHALL publish nothing.

#### Scenario: The payload of a new event
- **WHEN** Scout's event "Flight LIS → AMS" is created
- **THEN** `schedule.event_created` names its id, `travel`, its times and its email, and not "Flight LIS → AMS"

### Requirement: Removed Scout events are remembered
When a member removes an event Scout added, the module SHALL remember the
item it came from (message, item key, kind), so a later read of the same
message never adds it again, and SHALL keep it as a labelled example of a
wrong or unwanted read. Undoing the removal SHALL forget it.

#### Scenario: A delivery Inês didn't want
- **WHEN** Inês removes Scout's "Parcel from Lojas Sol" event
- **THEN** the item is remembered as removed and counts as a "no" example for `delivery`

### Requirement: Leaving and deleting
On `workspaces.member_left` the module SHALL delete the member's calendars,
events, removed items and settings. When `workspaces.workspace_deleting` is
published, the module SHALL delete the workspace's rows when its grace
period ends.

#### Scenario: Back in the workspace
- **WHEN** a member who left is invited back
- **THEN** they have an empty Campbooks calendar

#### Scenario: Purge
- **WHEN** a workspace is deleted with its grace period over
- **THEN** its calendars and events are gone and other workspaces' are kept
