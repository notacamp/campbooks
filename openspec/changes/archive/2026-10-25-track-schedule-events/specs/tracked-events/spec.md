## ADDED Requirements

### Requirement: The schedule module's changes are tracked
Every change the schedule module makes to a member's calendars, events or
settings SHALL be tracked as an event of the tracking plan, in the
transaction that makes it, with a snapshot read from the module's own rows
in that transaction: an event created (by the member, or by Scout from an
email), edited (which fields, by name), moved (with its times before),
cancelled by a later email, deleted, restored, answered (with the answer
before), added to a provider calendar and taken away again; a calendar
added by a grant, turned on or off, recoloured, or removed with its
connection; the member's settings changed (which settings, by name); and
each batch of a provider's changes the module applied (one summary per
batch, with counts, never one event per event). A change that changes
nothing SHALL track nothing. Events SHALL carry ids, enums, booleans,
counts and times only: never a title, a description, a place, notes, an
attendee, a calendar's name or colour, a time zone, a provider's id, an
etag or an iCalendar UID. Every mutating operation, job and handler of the
module SHALL have a Coverage row; where how a change ends at the provider
is already tracked by connectors, the row names that event.

#### Scenario: Moving an event
- **WHEN** a member changes the start of an event on their Campbooks calendar
- **THEN** one `schedule.calendar_event_moved` event is written with actor `user`, operation `schedule.update_event`, the event afterwards and its times before, and no title or place

#### Scenario: Renaming an event
- **WHEN** a member changes an event's title and place
- **THEN** one `schedule.calendar_event_edited` event is written naming the fields `title` and `place`, never their values, and sending the same values again tracks nothing

#### Scenario: Scout adds an event from an email
- **WHEN** a read of an email in a member's own inbox carries a flight
- **THEN** one `schedule.calendar_event_created` event is written as `system` with the event's kind `travel`, origin `scout` and the message's id

#### Scenario: A provider batch
- **WHEN** a batch of a provider calendar's changes creates two events and cancels one
- **THEN** one `schedule.provider_changes_applied` event is written with the calendar and the counts 2 created, 0 changed and 1 cancelled, and the same batch again tracks nothing
