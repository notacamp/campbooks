# schedule Specification

## Purpose
The `schedule` module keeps each member's Campbooks calendar and its events,
which hold the things Scout read from email. It owns event kinds, recurrence,
the member's time zone and working hours, and free time. Provider calendars
(Google, Zoho) and writing changes back to them arrive with `add-calendar-sync`.

## Requirements

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

### Requirement: Provider calendars
When `connectors.capability_granted` is published for the `calendar`
capability, the module SHALL create one calendar per provider calendar the
grant lists, owned by the granting member, with the provider's name, colour,
time zone and whether it may be written; the provider's primary calendar
SHALL sync and the others SHALL not until the member turns them on. A repeated
grant SHALL add only calendars the module does not have yet. Turning a
calendar's sync off SHALL hide its events and stop applying its changes;
turning it on SHALL ask connectors to read it again from the start
(`schedule.calendar_sync_changed`).

#### Scenario: A calendar grant
- **WHEN** Inês grants calendar access on her Google connection, which lists "Inês" (primary), "Family" and "Holidays in Portugal"
- **THEN** she has four calendars: her Campbooks calendar, "Inês" syncing, and "Family" and "Holidays in Portugal" not syncing

#### Scenario: The same grant again
- **WHEN** the same calendar grant is published again
- **THEN** no calendar is added or changed

#### Scenario: Turning a calendar on
- **WHEN** Inês turns "Family" on
- **THEN** connectors reads "Family" from the start and its events appear once the batch is applied

### Requirement: Provider event kinds
An event read from a provider SHALL be `none` unless Campbooks wrote it with a
kind, in which case it SHALL keep that kind and its source link.

#### Scenario: A meeting from Google
- **WHEN** a meeting Ana created in Google Calendar is applied
- **THEN** its kind is `none`

#### Scenario: A kind that round-trips
- **WHEN** Inês adds Scout's `travel` event to her Google calendar and Google sends it back on the next run
- **THEN** the event is still `travel` and still links its email

### Requirement: Applying calendar changes
The module SHALL consume `connectors.calendar_changed` v1 and apply each
batch to its calendar in order: an upsert creates or changes the event with
the provider's id and etag, a removal cancels it. A recurring series SHALL
be stored once with its recurrence rules as the provider gave them, and an
exception (one occurrence moved, changed or cancelled) as its own event
pointing at the series and the occurrence it replaces. Occurrences SHALL be
expanded when events are listed, within the asked window, and an exception
SHALL replace its occurrence. A batch for a calendar that does not sync
SHALL be ignored. A change to an event with a member's change still pending
SHALL keep the pending change.

#### Scenario: A weekly series
- **WHEN** a batch brings "Team call", every Monday 10:00 Lisbon time with no end
- **THEN** one event is stored, and listing 2–29 November shows four occurrences

#### Scenario: One occurrence moved
- **WHEN** the next batch brings an exception moving the 16 November occurrence to Tuesday 17 November 11:00
- **THEN** listing that week shows the call on Tuesday at 11:00 and not on Monday

#### Scenario: Deleted in Google
- **WHEN** a batch removes "Dentist"
- **THEN** the event is cancelled and `schedule.event_cancelled` is published

### Requirement: Scout's events meet provider calendars
When the member set an "also add to" calendar for a kind, an event Scout adds
of that kind SHALL also be pushed there as by "Add to my calendar". The
setting SHALL refuse a calendar that is not the member's or cannot be written.
When an item came from an iCalendar part whose `UID` an event on one of the
member's syncing calendars already has, the module SHALL link that event to
the message instead of adding one.

#### Scenario: Also added to Google
- **WHEN** Inês set `travel` to also go to her Google calendar "Inês" and a flight is read
- **THEN** the event is on her Campbooks calendar and a create is requested for "Inês"

#### Scenario: An invite already in Google
- **WHEN** an invitation's iCalendar part has the `UID` of an event Google already put on Inês's primary calendar
- **THEN** that event links the email and no second event is added

### Requirement: Changing provider events
A member SHALL be able to create an event on a provider calendar that can be
written, change a single event or one occurrence of a series, delete an event
with an undo, and answer an invitation (accept, tentative, decline). A change
to an event on a provider calendar SHALL be stored at once, marked pending,
and published as `schedule.outbound_requested` v1 in the same transaction,
carrying the connection, the provider's ids, the kind of change and the etag
the member's version was based on; the event's text SHALL travel as a blob. A
delete of a provider event SHALL reach the provider only when the undo window
(10 seconds) has passed; after it, `restore_event` SHALL ask for the event to
be made again.

#### Scenario: Moving the dentist
- **WHEN** Inês moves "Dentist" on her Google calendar from 15:00 to 16:00
- **THEN** the event shows 16:00, pending, and an update is requested with the etag she saw

#### Scenario: One occurrence of a series
- **WHEN** Inês moves only the 23 November occurrence of "Team call"
- **THEN** an exception is requested for that occurrence and the other Mondays are unchanged

#### Scenario: Read-only calendar
- **WHEN** Inês tries to create an event on "Holidays in Portugal", which cannot be written
- **THEN** the answer is 400 and nothing is requested

#### Scenario: Deleted and undone in time
- **WHEN** Inês deletes a Google event and presses Undo within 10 seconds
- **THEN** the event is back as it was and no delete reaches Google

#### Scenario: Accepting an invitation
- **WHEN** Inês accepts Pedro's invitation to "Site visit, Rua das Flores"
- **THEN** her response is stored and requested for Google

### Requirement: Add to my calendar
A member SHALL be able to push an event of their Campbooks calendar to one of
their provider calendars that can be written. The module SHALL request its
creation there and, once created, keep one event: on the provider calendar,
with its kind, its source link and `pushed_from` naming the Campbooks event,
which leaves the Campbooks calendar. The last calendar a member pushed to
SHALL be offered first next time. `schedule.unpush_event` SHALL remove the
provider copy (asked of connectors) and keep the Campbooks event, which is
back on the Campbooks calendar; it is the undo of `push_event`, and its own
undo is `push_event` with the same calendar. Deleting a pushed event SHALL
delete it at the provider, with Undo. A copy the provider does not take SHALL
go away and the Campbooks event SHALL be back, with a notice that it was not
saved.

#### Scenario: A concert to Google
- **WHEN** Inês adds Scout's `show` event "Mariza at Coliseu" to her Google calendar "Inês"
- **THEN** one event remains, on "Inês", still a `show`, still linking the ticket email

#### Scenario: Changed her mind
- **WHEN** Inês takes the concert off her Google calendar with `unpush_event`
- **THEN** it is deleted at Google and "Mariza at Coliseu" is on her Campbooks calendar again

### Requirement: Pushing and unpushing answer with what undoes them
`push_event` and `unpush_event` SHALL be in the operation catalogue with
`assistant: true`, `mutates: true`, `risk: low` and scope `schedule:write`,
`push_event` ↔ `unpush_event` as undo pair. `push_event`'s confirmation SHALL
name the calendar it writes to from `list_calendars`, never from the model's
words. `push_event` SHALL answer with the `id` of the copy, to take away with
`unpush_event`; `unpush_event` with the `id` of the Campbooks event and the
`calendar_id` it was removed from, to push again.

#### Scenario: Adding to Google from the chat
- **WHEN** Scout proposes adding the concert to Google
- **THEN** the confirmation says it will be written to the calendar "Inês"

#### Scenario: Undo adding to Google
- **WHEN** Inês adds a concert to her Google calendar and then calls `unpush_event` on the copy
- **THEN** the copy is deleted at Google, the Campbooks event is on her Campbooks calendar again, and the answer names the Google calendar to push to again

### Requirement: The outcome of a change at the provider
When connectors reports a calendar change `applied`, the module SHALL store
the provider's new id and etag and clear the pending mark. When the provider
refused it because the event had changed there (412), the module SHALL keep
the provider's version, clear the pending mark, and mark the event so the
member is told their change was not kept. Any other failure SHALL clear the
pending mark, restore the event's last provider version and mark it the same
way. A failure SHALL be recorded by kind only.

#### Scenario: Changed in Google meanwhile
- **WHEN** Inês moves "Dentist" to 16:00 while the clinic moved it to 17:00 in Google
- **THEN** the event shows 17:00 and says her change was not kept because it changed in Google

### Requirement: Calendar access removed
On `connectors.connection_removed` the module SHALL delete the calendars of
that connection and their events, keeping the member's Campbooks calendar.

#### Scenario: Calendar access removed
- **WHEN** Inês disconnects her Google connection
- **THEN** her Google calendars and their events are gone and her Campbooks calendar is kept

### Requirement: Whose calendar it is
A provider calendar SHALL carry the provider's name (`google` or `zoho`) when
the calendar list of its grant names one, and the calendar view SHALL show it,
so a client can say "Google" and not "the provider". A calendar made before
the list named one SHALL have none.

#### Scenario: The provider on a calendar
- **WHEN** Inês grants calendar access on her Google connection
- **THEN** each of her Google calendars shows `google` as its provider and her Campbooks calendar shows none

### Requirement: Which event an item became
An event view SHALL list the keys of the read items it stands for, so that a
reader can tell which event an item became without comparing titles or times.

#### Scenario: The dentist's email
- **WHEN** Scout added "Dentist" from the item `<message>:model:item:0`
- **THEN** the event lists that key and the reader shows the item as added

### Requirement: Events made from an email
A client that makes an event from an email MAY name the email
(`conversations.message`) and its conversation. The module SHALL accept it
only when the caller may read that message, and answer 404 when not; the
event then leads back to the email like one Scout added.

#### Scenario: Add by hand from a flagged email
- **WHEN** Inês adds an event from an email she can read
- **THEN** the event keeps the link to that email and its sheet offers to open it

#### Scenario: An email she cannot read
- **WHEN** a request names an email Inês cannot read
- **THEN** it is answered with 404 and no event is made
