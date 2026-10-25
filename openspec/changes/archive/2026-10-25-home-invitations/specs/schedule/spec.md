## ADDED Requirements

### Requirement: Invitations awaiting an answer
An event SHALL await its member's answer while it is on one of their
provider calendars that syncs, their response is `needs_action`, they do not
organise it, it is a single event or a series (not one occurrence of a
series), and it is not cancelled, deleted, pushed away or started (an all-day
event: until its first day ends in the member's zone). The module SHALL
announce `schedule.invitation_changed` v1, in the transaction of every
change that can make an event await an answer or stop it awaiting one, when
it awaits one now or did before: the workspace, event, calendar and member,
a revision that grows with every announcement of that event, and either the
invitation (the day it starts in the member's zone, when it stops awaiting
an answer, whether a person rather than a machine organises it, from the
organiser's address alone) or why it no longer awaits one (`answered`,
`cancelled`, `removed`, `hidden`, `started`). It SHALL carry no title,
place, name or address. The organiser a provider gives SHALL be kept with
the event and shown on it (`organizer`), except to the member who organises
it. Events stored before invitations were announced SHALL be announced once,
from the module's own rows, with no resync and no model.

#### Scenario: Offsite planning
- **WHEN** a batch brings Ana's Google event "Offsite planning" on Thursday 14:00 that João organises and Ana has not answered
- **THEN** an invitation is announced for Ana, on Thursday, lapsing at 14:00, from a person, and the event shows João as its organiser

#### Scenario: Not invitations
- **WHEN** a batch brings an event Ana organises, one she accepted and one that started five minutes ago, all with her response `needs_action` but the accepted one
- **THEN** none is announced

#### Scenario: A webinar from a service
- **WHEN** the organiser's address is `no-reply@webinars.example.com`
- **THEN** the invitation is announced as not from a person

#### Scenario: Closed
- **WHEN** Ana's open invitation is cancelled at Google, its calendar turned off, or its connection removed
- **THEN** it is announced closed as `cancelled`, `hidden` or `removed`, with a newer revision

#### Scenario: Before this change
- **WHEN** a workspace holds an unanswered future invitation stored before invitations were announced
- **THEN** it is announced once, within a minute of the deploy

## MODIFIED Requirements

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
be made again. An answer to an invitation on a provider calendar SHALL
likewise reach the provider only when the undo window has passed
(`schedule.release_response`), with the answer the event holds then: within
the window a new answer SHALL replace it and "not answered" SHALL withdraw
it, so nothing reaches the provider; once it was asked of the provider,
another answer SHALL wait for it to finish (`change_pending`).

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
- **THEN** her response is stored, pending, and requested for Google once 10 seconds have passed

#### Scenario: Accepted and undone in time
- **WHEN** Inês accepts an invitation and presses Undo within 10 seconds
- **THEN** it is not answered again, not pending, and no answer reaches Google

#### Scenario: Changed her mind in time
- **WHEN** Inês declines, then says maybe within 10 seconds
- **THEN** only "maybe" is requested for Google
