## MODIFIED Requirements

### Requirement: The reply slot
The reader SHALL expose a slot at the end of the thread for the send slice's
composer, with Reply (R), Reply all (Shift+R) and Forward (F) that tell the slot
which message to answer and how; without a composer those actions SHALL be
hidden. Reply SHALL first open the reply dialog (Scout's likely replies and
draft); its "Write my own" and "Open in composer" SHALL open the slot's
composer on the newest message, the latter on the draft Scout wrote. Reply
all and Forward SHALL open the composer directly.

#### Scenario: No composer yet
- **WHEN** the inbox runs without a reply-slot renderer
- **THEN** the reader shows no Reply, Reply all or Forward

#### Scenario: Write my own
- **WHEN** the user presses R in the reader, then chooses "Write my own"
- **THEN** the dialog closes and the composer opens at the end of the thread, as before
