## MODIFIED Requirements

### Requirement: Labelled actions on the selected item
The selected item SHALL show its actions directly under it, all in one
button style, each its label and its key, in two groups split by a thin
divider: the item's own actions (keys 1 to 3, at most three) and the
universal ones (Done E, Snooze… S, Not needed X). A need offers Reply; a
question (`check`) offers Yes and No and Snooze, with no Done until it is
answered and no Not needed beside its No; a warning offers It's safe, Done
and Snooze. Other rows SHALL show no buttons and no key hints. An action is
`{id, label, ai?, key}`; a need the server ranked offers its own actions
instead (Scout's ranked actions). A button for an action that runs the AI SHALL carry the visor
before its label, never a sparkle.

#### Scenario: The selected row
- **WHEN** Marta's reply is selected
- **THEN** under it are "Reply 1", a divider, "Done E", "Snooze… S" and "Not needed X", and no other row shows a button or a key

## ADDED Requirements

### Requirement: Scout's ranked actions
A need that carries `actions` (the catalogue is on and its message was
rated) SHALL offer them as its own actions, in the server's order, on keys 1
to 3, in the same row, with the visor before them saying Scout picked them
(never a sparkle); the universal actions SHALL stay as they are. A need
without the `actions` key SHALL offer exactly the fixed actions above; only
a missing key falls back. A question (`check`) SHALL keep Yes and No, a
bill already said to be still to pay SHALL never offer Still to pay again,
and a bill's Already paid and Still to pay, a bill still to pay's Mark paid
and a warning's Report and delete and It's safe SHALL always be there, first
when the ranking left them out; a warning SHALL offer only Report and delete
and It's safe, whatever the ranking holds. Reply, Report and delete, It's safe, Already paid and Still to pay SHALL do
what the fixed actions do, with the same confirmation and Undo. Remind SHALL
open the Snooze dialog as "Remind me about <name>…", a bill's reminder
before its due date first, and snooze it with Undo. Forward SHALL open a
dialog that finds people by name (the people replied to most before
anything is typed, the server's matches from two characters), never an
address typed from memory, and picking one SHALL open the conversation with
a forward draft addressed to them; nothing is sent from Home. A forward of
that email nobody wrote in yet (no text of the member's own, no copies, no
files, at most one person) SHALL be addressed again rather than doubled; any
other SHALL stay as it was and a new one start. When the email or its drafts
can't be read, the dialog SHALL say so and let another pick try; closing it
while the forward starts SHALL go nowhere. Open
attachment SHALL open the item with its email's first file focused, and say
so when there is no file to open. A pair
the server offers as one SHALL be one button that runs both, in order. An
inbox the member may only read SHALL offer only Remind and Open attachment
among them. A pair of two actions that ask a follow-up
in a dialog SHALL never be offered, and Open attachment before a dialog only
opens the item.

#### Scenario: Ranked
- **WHEN** the phishing warning carries It's safe (81) then Report and delete (19)
- **THEN** its own actions are "It's safe 1" then "Report and delete 2", the visor before them, and 1 marks it safe

#### Scenario: Not rated
- **WHEN** a need has no `actions` key
- **THEN** it offers exactly the fixed actions, with no visor before them

#### Scenario: Forward by name
- **WHEN** the user presses Forward on Marta's need and types "joao"
- **THEN** only João Matos is listed, and picking him opens Marta's conversation with a forward draft to him

#### Scenario: A pair
- **WHEN** a need carries the pair Open attachment and Forward
- **THEN** it shows one button, "Open attachment, then Forward…", that does both
