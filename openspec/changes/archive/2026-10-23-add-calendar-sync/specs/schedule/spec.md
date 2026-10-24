## Purpose

Provider calendars in the `schedule` module: the Google and Zoho calendars a
calendar grant brings, their events applied and kept in step, the member's
changes written back to them, and Scout's events copied onto them when the
member says so. Builds on the Campbooks calendar and the typed events of
`add-schedule-module`.

## ADDED Requirements

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
