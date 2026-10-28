## ADDED Requirements

### Requirement: A task that asks Done? stays on Home
The projection of a task SHALL keep `asks_done` from `tasks.task_changed` (false
when the event has none) with the item, and clear it when the task closes.
`attention.list` SHALL count a task that asks Done? as for today whatever its
plan, so it is listed and not left out under "more in Tasks"; its band follows the
existing rules.

#### Scenario: Planned for next week, then Ana replies
- **WHEN** a task planned nine days ahead is not on Home and `task_changed` says it asks Done?
- **THEN** its item is listed

#### Scenario: Not yet
- **WHEN** a later `task_changed` says it no longer asks
- **THEN** the plan decides again, and an older event replayed changes nothing
