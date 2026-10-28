## ADDED Requirements

### Requirement: Mine and following
Every calendar SHALL have a role: **mine**, **following** or **off**. Mine
SHALL mean the calendar is shown and is the member's own: its busy events
count for free time, time for a task may be held on it when it can be
written, and its invitations await the member's answer. Following SHALL
mean the calendar is shown and nothing else. Off SHALL mean it is not shown
and its changes are not applied, as a calendar whose sync is off. The
Campbooks calendar SHALL always be mine. A provider calendar SHALL carry
whether the provider says the account owns it (`owned`: true, false, or
unknown), and the calendar view SHALL show its `role` and `owned`. The member
SHALL change a provider calendar's role with the calendar PATCH (`role`);
`syncing` SHALL stay accepted and mean "not off". A patch whose `role` and
`syncing` disagree, or that makes the Campbooks calendar anything but mine,
SHALL be refused as `invalid_calendar`. Going from off to mine or following
SHALL ask connectors to read the calendar from the start; going between mine
and following SHALL NOT. Calendars made before roles SHALL be mine when they
sync and can be written, following when they sync and cannot be written, and
off otherwise, keeping whether they come back mine or following.

#### Scenario: A colleague's calendar
- **WHEN** Inês turns on "Marta", a calendar Marta shared with her, and sets it to following
- **THEN** Marta's events show on Inês's calendar, they do not make Inês busy, no time can be held on it, and none of its invitations is on Inês's Home

#### Scenario: Back to mine
- **WHEN** Inês sets "Family" from following to mine
- **THEN** its busy events count for her free time again, its pending invitations are announced again, and connectors is not asked to read it again

#### Scenario: The Campbooks calendar stays mine
- **WHEN** a PATCH sets Inês's Campbooks calendar to following
- **THEN** the answer is 400 `invalid_calendar` and nothing changes

#### Scenario: Before roles
- **WHEN** roles arrive and Inês has "Inês" (syncing, writable), "Holidays in Portugal" (syncing, read-only) and "Team" (not syncing, writable)
- **THEN** "Inês" is mine, "Holidays in Portugal" is following, and "Team" is off and comes back mine when turned on

## MODIFIED Requirements

### Requirement: Free time
The module SHALL answer, for a member, a window and a duration, the free
slots within the member's working hours, in their time zone, that no busy
event of one of the member's own calendars (their Campbooks calendar and
the provider calendars whose role is mine) overlaps; cancelled events,
all-day events and the events of calendars the member follows or has off
SHALL NOT count. The same answer SHALL be available to other modules through
the kernel port `Availability::free_slots(member, window, duration)` and to
the member as `schedule.free_slots`, and the answer SHALL name the calendars
it counted.

#### Scenario: A free hour on Tuesday
- **WHEN** Inês works 09:00–18:00 and on Tuesday has events 09:00–12:30 and 14:00–18:00, and asks for an hour that day
- **THEN** the answer is 12:30–14:00, naming her Campbooks calendar as the calendar counted

#### Scenario: A weekend
- **WHEN** Inês has no working hours on Saturday and asks for an hour on Saturday
- **THEN** the answer is empty

#### Scenario: A followed calendar does not make her busy
- **WHEN** Inês follows "Marta", which has a busy event on Tuesday 12:30–14:00, and asks for an hour on Tuesday as above
- **THEN** the answer is still 12:30–14:00, and "Marta" is not among the calendars counted

### Requirement: Provider calendars
When `connectors.capability_granted` is published for the `calendar`
capability, the module SHALL create one calendar per provider calendar the
grant lists, owned by the granting member, with the provider's name, colour,
time zone, whether it may be written and whether the account owns it; a
calendar the account does not own SHALL be following and any other mine, and
the provider's primary calendar SHALL sync while the others SHALL be off
until the member turns them on. A repeated grant SHALL add only calendars
the module does not have yet, and SHALL record whether the account owns a
calendar the module has when it did not know, changing nothing else.
Turning a calendar off SHALL hide its events and stop applying its changes;
turning it on SHALL ask connectors to read it again from the start
(`schedule.calendar_sync_changed`).

#### Scenario: A calendar grant
- **WHEN** Inês grants calendar access on her Google connection, which lists "Inês" (primary, owned), "Family" (owned) and "Holidays in Portugal" (not owned)
- **THEN** she has four calendars: her Campbooks calendar, "Inês" mine and syncing, "Family" off and mine when turned on, and "Holidays in Portugal" off and following when turned on

#### Scenario: The same grant again
- **WHEN** the same calendar grant is published again
- **THEN** no calendar is added or changed

#### Scenario: A grant that knows more
- **WHEN** Inês reconnects her Google calendar access and the list now says she does not own "Holidays in Portugal", which the module had with ownership unknown and set to mine
- **THEN** "Holidays in Portugal" shows as not owned and stays mine

#### Scenario: Turning a calendar on
- **WHEN** Inês turns "Family" on
- **THEN** connectors reads "Family" from the start and its events appear once the batch is applied

### Requirement: Holds for tasks
The schedule module SHALL implement the kernel `Holds` port: `hold` SHALL make
an event of kind `hold`, busy, made by the member, linked to the task, on one
of the member's own writable calendars (their Campbooks calendar when none is
named), asking a provider calendar for it like any event the member makes, and
publish `schedule.event_created`; `release` SHALL remove it as `delete_event`
does and publish `schedule.event_cancelled`, and SHALL do nothing for an event
that is gone or is not that member's hold. A calendar that is not the member's
SHALL be refused as not found, one they follow as `calendar_followed`, and one
that cannot be written as `calendar_read_only`.

#### Scenario: On Google
- **WHEN** the tasks module holds 10:00–10:30 on Ana's Google work calendar
- **THEN** the hold event is stored, marked pending, and asked of Google in the same transaction

#### Scenario: Someone else's calendar
- **WHEN** a hold names a calendar of another member
- **THEN** nothing is made and the call answers not found

#### Scenario: A followed calendar
- **WHEN** a hold names "Marta", a writable calendar Ana follows
- **THEN** nothing is made and the call answers `calendar_followed`

### Requirement: Invitations awaiting an answer
An event SHALL await its member's answer while it is on one of their
provider calendars that syncs and is theirs (role mine), their response is
`needs_action`, they do not organise it, it is a single event or a series
(not one occurrence of a series), and it is not cancelled, deleted, pushed
away or started (an all-day event: until its first day ends in the member's
zone). The module SHALL announce `schedule.invitation_changed` v1, in the
transaction of every change that can make an event await an answer or stop
it awaiting one (a calendar's role among them), when it awaits one now or did
before: the workspace, event, calendar and member, a revision that grows with
every announcement of that event, and either the invitation (the day it
starts in the member's zone, when it stops awaiting an answer, whether a
person rather than a machine organises it, from the organiser's address
alone) or why it no longer awaits one (`answered`, `cancelled`, `removed`,
`hidden`, `started`). It SHALL carry no title, place, name or address. The
organiser a provider gives SHALL be kept with the event and shown on it
(`organizer`), except to the member who organises it. Events stored before
invitations were announced SHALL be announced once, from the module's own
rows, with no resync and no model.

#### Scenario: Offsite planning
- **WHEN** a batch brings Ana's Google event "Offsite planning" on Thursday 14:00 that João organises and Ana has not answered
- **THEN** an invitation is announced for Ana, on Thursday, lapsing at 14:00, from a person, and the event shows João as its organiser

#### Scenario: Not invitations
- **WHEN** a batch brings an event Ana organises, one she accepted and one that started five minutes ago, all with her response `needs_action` but the accepted one
- **THEN** none is announced

#### Scenario: A webinar from a service
- **WHEN** the organiser's address is `no-reply@webinars.example.com`
- **THEN** the invitation is announced as not from a person

#### Scenario: A followed calendar's invitations
- **WHEN** a batch brings, on "Marta" which Ana follows, an event Marta has not answered
- **THEN** none is announced

#### Scenario: Closed
- **WHEN** Ana's open invitation is cancelled at Google, its calendar is turned off or set to following, or its connection is removed
- **THEN** it is announced closed as `cancelled`, `hidden` (for either the calendar turned off or set to following) or `removed`, with a newer revision

#### Scenario: Before this change
- **WHEN** a workspace holds an unanswered future invitation stored before invitations were announced
- **THEN** it is announced once, within a minute of the deploy
