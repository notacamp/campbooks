## ADDED Requirements

### Requirement: Event cards
An event proposal SHALL show as a card in the member's language that names
the event and its calendar from the record (title and time, never the model's
words) and says what the change does from the fields it really sends: a new
start moves it, a new end alone ends it there, a new title alone renames it,
an answer, an addition to a named calendar or a removal. Confirm SHALL send the
request only when its operation is in the client's own list (`PATCH` for
`schedule.update_event` as for a task's edit), and Undo SHALL send the undo with
the path parameters its `@<path parameter>:<field>` entries name filled from
the change's answer, each one plain id; when the answer gives none the undo is
not sent and the card has no Undo. After a reload a settled card shows its
state without Undo for a change whose undo needs the answer.

#### Scenario: A new event's Undo
- **WHEN** Sam confirms a created event and presses Undo
- **THEN** the event the create answered is deleted

#### Scenario: A rename
- **WHEN** Scout proposes only a new title for "Dentist"
- **THEN** the card reads "Rename this event" with the new title quoted
