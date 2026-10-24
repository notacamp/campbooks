## MODIFIED Requirements

### Requirement: Quick add
`n t` (new task, from anywhere in the app), the "New task" button in the
Tasks list's header and the empty state's "Add a task" SHALL open Tasks with
the new-task form in the right pane, at `/w/{id}/tasks?new=1`, in place of a
task's detail; below `lg` it SHALL replace the list, with Back. The form SHALL
take the shape of a task's detail: the title as the page's title ("What needs
doing?", focused), Do or Pay, a "Due" section with the due chips Today,
Tomorrow, the next weekday and Pick a date (which shows the month grid in
place), then "Add task" (Enter) and Cancel (Escape). Adding SHALL create the
task, show "Task added" with Undo, keep the form open and empty with the title
focused for the next one, and show the task in the list. Escape, Cancel,
Close and Back SHALL close the form and give the focus back to "New task";
opening a task SHALL close it. Dates SHALL NOT be read from the typed words.

#### Scenario: Adding for tomorrow
- **WHEN** Ana presses N then T, types "Renew the parking permit", picks Tomorrow and presses Enter
- **THEN** the task appears under Coming up, due tomorrow, and the form stays open with the title empty and focused

#### Scenario: The form in the right pane
- **WHEN** Ana presses "New task" in the list's header
- **THEN** the right pane shows the form instead of a task, the address ends in `?new=1`, and the list stays beside it

#### Scenario: Closing the form
- **WHEN** Ana presses Escape in the form
- **THEN** the form closes, the right pane shows "Choose a task" again and the focus is on "New task"

#### Scenario: A date from the grid
- **WHEN** Ana presses Pick a date in the form and picks 20 October in the month grid
- **THEN** the chip shows that day and the task is created due on 20 October; browsing months changes nothing

#### Scenario: Phone
- **WHEN** Ana opens the form on a 375 px screen
- **THEN** it replaces the list with Back, and nothing scrolls sideways
