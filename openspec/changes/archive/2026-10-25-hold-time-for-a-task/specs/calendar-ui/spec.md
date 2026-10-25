## ADDED Requirements

### Requirement: Held time on the calendar
A held block SHALL show as a Held event ("Held · <title>") on its calendar,
and opening it SHALL offer Open task. A held task SHALL have no Planned row,
and no Due row on the day it is held when that is its due day. The task sheet
of a Do task SHALL offer Hold time (`h`) with the same panel as the task's
detail. Removing the held block SHALL release the task's hold, with Undo
restoring both.

#### Scenario: Held on the planned day
- **WHEN** "Send the signed lease" is held Wednesday 10:00–10:30 and due Thursday
- **THEN** Wednesday shows the Held block at 10:00 and no Planned row, and Thursday shows the Due row

#### Scenario: Removing the block
- **WHEN** Ana removes the held block from the calendar and presses Undo
- **THEN** the block is back and the task shows "Held Wed 10:00–10:30" again
