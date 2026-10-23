## ADDED Requirements

### Requirement: Scout chat reads tasks
A chat SHALL be able to be about a task (`tasks.task`, read through
`tasks.get`), and Scout SHALL have the read-only tools `my_tasks` (over
`tasks.list`) and `read_task` (over `tasks.get`), run as the asking member so
that a task they may not see answers as not visible. Each result SHALL hold at
most 8 KiB, keep the API's dates (RFC 3339 times) and name tasks by RecordRef.
A task's title SHALL never be logged.

#### Scenario: What do I have due this week?
- **WHEN** Ana asks in a chat "what do I have due this week?" on Monday 19 October
- **THEN** Scout calls `my_tasks` with `due_from` 2026-10-19 and `due_to` 2026-10-25 and answers from her own open tasks, saying the overdue ones apart

#### Scenario: A task she cannot see
- **WHEN** Ana's chat is about a task that is João's and was never handed to her
- **THEN** `read_task` answers "not visible" and Scout says it cannot see it
