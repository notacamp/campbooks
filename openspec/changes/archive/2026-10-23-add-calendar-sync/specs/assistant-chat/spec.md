## ADDED Requirements

### Requirement: Event actions and their undos
`propose_event_change` SHALL propose one change to a calendar event, as one
row of the action table: `create` (`schedule.create_event`, undo
`schedule.delete_event` of the event the answer names), `update`
(`schedule.update_event`, a restoring row whose undo is the same operation
with the event's values before), `delete` (`schedule.delete_event`, undo
`schedule.restore_event` with the `undo_token` of the answer), `respond`
(`schedule.respond_event`, a restoring row), `push` (`schedule.push_event`, undo
`schedule.unpush_event`) and `unpush` (undo
`schedule.push_event`). Restoring an event is not an action of its own: it is
only the undo of a delete. Each operation and undo SHALL be `assistant`,
`mutates` and `risk: low` in the catalogue, checked by a test. The values a
restoring undo writes back SHALL be the event's, read as the member when the
proposal is checked, never the model's arguments.

#### Scenario: A move put back
- **WHEN** the model proposes moving an event from 15:00 to 16:00
- **THEN** the undo is `schedule.update_event` with the event's own 15:00 start and end

#### Scenario: A delete put back
- **WHEN** Ana confirms a delete and then Undo
- **THEN** the undo sends `schedule.restore_event` with the `undo_token` the delete answered

### Requirement: Checking an event proposal
The server SHALL read the event and the member's calendars as the member and
SHALL refuse with a tool result naming why: `not_visible`; `event_not_changeable`
for an event that waits for its provider, a series, or one on a calendar that
cannot be written; `not_an_invitation` for an answer to an event that is not
an invitation the member answers; `already_so`; `no_such_calendar` for a push
to a calendar the member does not have, that cannot be written or is their
Campbooks calendar; `start_out_of_range` for a new event that starts before now
or a year or more away (a day is judged in the member's days);
`invalid_when` for an edit that cannot be written: a day on a timed event, a
moment on an all-day event, or an end before the start. A card SHALL name the
event and the calendar from their records, never the model's words, and an
edit's detail SHALL hold only the fields that really change, compared with the
event as read.

#### Scenario: A rename that is not a move
- **WHEN** the model proposes `update` with a new title and the event's own start
- **THEN** the card's detail holds the title only, so it reads as a rename

#### Scenario: A time that cannot be written
- **WHEN** the model proposes `update` with a day for a timed event
- **THEN** the model is told `invalid_when` and no card is stored

#### Scenario: A year in the past
- **WHEN** the model proposes `create` starting in 2020
- **THEN** the model is told `start_out_of_range`

## MODIFIED Requirements

### Requirement: What a proposal holds
A proposal SHALL be stored with Scout's message (`proposals`) and SHALL
hold its id, its kind (`change`, `rule` or `reply`), the conversation as a
record reference, the conversation's facts read from its record (sender,
subject, stream; never the model's words), and its state (`pending`,
`done`, `declined`, `failed`, `undone`). A change proposal SHALL also hold
its action, its detail (`stream`, `label` or `until`, and for an event its
title, start, end, place, answer or calendar name), and the request and
the undo the server built from the catalogue (operation, method, a path
relative to the workspace's API root, and a body). An undo MAY name
top-level fields of the request's answer it takes (`from_response`), which
the client copies into the undo body and nothing else, and entries
`@<path parameter>:<field>` that fill a path parameter the request's path
holds as `{<path parameter>}` with the named top-level field of the answer
(the undo of a new event is addressed to the id the create answered). A rule
proposal SHALL hold its sentence and a reply proposal its intent, and neither
a request.

#### Scenario: Facts, not words
- **WHEN** the model proposes archiving a conversation and writes "the urgent invoice from the bank" in its text
- **THEN** the proposal's facts are the conversation's own sender and subject

#### Scenario: An undo addressed by the answer
- **WHEN** a proposed create is confirmed and answers the new event's id
- **THEN** its undo is the delete of that event, addressed with the id from the answer
