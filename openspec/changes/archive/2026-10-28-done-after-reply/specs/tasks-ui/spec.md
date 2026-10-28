## ADDED Requirements

### Requirement: The Tasks place asks Done? after a reply
A task that asks Done? SHALL show "You replied · Done?" under its title in the
list. Its detail SHALL ask "You replied to <name> at <time>. Is this task done?"
above "Where it came from", with Done (1) and Not yet (2) as large buttons, and
"Where it came from" SHALL list "Your reply · <date>". Both answers SHALL have an
Undo.

#### Scenario: Done from the detail
- **WHEN** Ana opens the task and presses 1
- **THEN** the task is done, with an Undo
