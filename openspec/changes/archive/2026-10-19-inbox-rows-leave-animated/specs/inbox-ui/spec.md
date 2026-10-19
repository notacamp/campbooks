## ADDED Requirements

### Requirement: Rows leaving the list say what happened
When the member's own action takes a conversation out of the list on screen
(Done, Move to Inbox, Delete, Restore, Report spam, Not spam, or a move to
another stream), its row SHALL first say what happened, in place: an icon and
"Done", "Moved to Inbox", "Deleted", "Restored", "Reported as spam", "Not
spam" or "Moved to <stream>", followed by the subject, wiping in over the row.
Report spam and Delete SHALL use the danger colour, the others the success
colour. The row SHALL then fold away while the rows below close the gap, all
within `--duration-confirm`. A leaving row SHALL NOT be a button, SHALL be
skipped by the keyboard cursor, and SHALL be hidden from assistive tech (the
toast says the same). Rows leaving one after another SHALL keep their order.
Undo while a row folds SHALL put the conversation straight back. Under reduced
motion the row SHALL go at once. Rows that leave for any other reason (another
device, a sync) SHALL go at once.

#### Scenario: Done from the list
- **WHEN** the user presses E on the row under the cursor in Primary
- **THEN** the row says "Done" with the subject, folds away under a second, the rows below move up, and the cursor is on the next row

#### Scenario: Report spam in red
- **WHEN** the user presses ! on a row in Primary
- **THEN** the row says "Reported as spam" in the danger colour, then folds away

#### Scenario: Moved to another stream
- **WHEN** the user moves an open conversation from Primary to Newsletters
- **THEN** its row in Primary says "Moved to Newsletters", then folds away

#### Scenario: Undo while it folds
- **WHEN** the user presses # on a row, then Z before the row has gone
- **THEN** the conversation is back in the list as a normal row
