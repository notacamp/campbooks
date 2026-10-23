## ADDED Requirements

### Requirement: Projecting tasks
The handler `attention.project_task` SHALL consume `tasks.task_changed` and
keep one item per task: reason `do` or `pay` from the task's verb, trust
`sure`, the task's due date, the assignee's container (visible to that member
only), and the task's source as its cause when it has one. It SHALL clear the
item when the task is done, not needed or handed off, apply newer revisions
only, and leave a tombstone for a clear that arrives before its open.

#### Scenario: A task for Ana
- **WHEN** `task_changed` reports an open `pay` task assigned to Ana, due Friday
- **THEN** Ana has a `pay` item due Friday for that task, and João does not see it

#### Scenario: Handed to João
- **WHEN** Ana hands the task to João
- **THEN** Ana's item is cleared and João has one for the same task

#### Scenario: Out of order
- **WHEN** the `done` revision of a task is delivered before its `created` revision
- **THEN** the task has no open item

### Requirement: Tasks put off until later
An item for a task planned Later SHALL be left out of `attention.list` until
its Later time, and SHALL come back by itself once that time has passed.

#### Scenario: Monday morning
- **WHEN** a task is planned Later until Monday 09:00
- **THEN** it is not listed on Sunday, and it is listed on Monday at 09:01

### Requirement: A task replaces its conversation's item
While a task whose source is a conversation is open, `attention.list` SHALL
leave out that conversation's `do` or `pay` item for the task's assignee.

#### Scenario: Made a task from the lease email
- **WHEN** Ana makes a task from a conversation that has a Do need
- **THEN** Home lists the task once and does not list the conversation's Do item
