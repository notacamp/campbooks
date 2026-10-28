## ADDED Requirements

### Requirement: You replied, is it done?
A task that asks Done? SHALL read on Home as: "You replied" and when, "Task · Do ·
from <name>", the task's title, and "You answered <name>'s email. Is this task
done?". Selected, its actions SHALL be Done (1) and Not yet (2), then Open the
thread (O) and Ask Scout (`.`), with their keys shown; opened, it SHALL say why
Scout asks ("You replied in the same thread at <time>, after the task was made.
Scout didn't read your reply.", "Nothing changes until you answer."). Done SHALL
confirm "Done · <title>" and Not yet "Okay, still open · <title>", each with an Undo.
On phones Done and Not yet SHALL be side by side without scrolling sideways.

#### Scenario: Not yet
- **WHEN** Ana presses 2 on the row
- **THEN** it folds with "Okay, still open · Send the signed lease", Z brings it back, and Home does not ask again until she replies again

#### Scenario: The thread
- **WHEN** Ana presses O on the row
- **THEN** the conversation the task came from opens
