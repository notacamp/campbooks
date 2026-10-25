## ADDED Requirements

### Requirement: The tasks module's changes are tracked
Every change the tasks module makes to a task SHALL be tracked as an event
of the tracking plan, in the transaction that makes it, with a snapshot
read from the module's own row in that transaction: a task created (by a
member, or by Scout from an ask in an email), edited (which fields, by
name), its due date changed (with the date before), planned (with the plan
before), done, not needed, reopened (with the state before), and
reassigned: handed off to a teammate or taken back (with the member who
had it). A change that
changes nothing SHALL track nothing, including a plan set again to what it
was. Events SHALL carry ids, enums, booleans, counts, dates and times only:
never a title, a sender's address, an amount or a currency, a quote from
the email or a person's name; members appear by their user id. Every
mutating operation, job and handler of the module SHALL have a Coverage
row.

#### Scenario: A member writes a task
- **WHEN** a member makes a task with a title and a due day
- **THEN** one `tasks.task_created` event is written with actor `user`, operation `tasks.create`, the task's id, verb, origin `own`, state `open` and due day, and no title

#### Scenario: Scout reads an ask
- **WHEN** a read of an email in a member's own inbox carries a `pay` ask with an amount
- **THEN** one `tasks.task_created` event is written as `system` with origin `ask`, verb `pay`, the conversation it came from by reference and `has_amount` true, and no title, amount or currency

#### Scenario: Renaming a task and moving its due date
- **WHEN** a member changes a task's title and its due day in one update
- **THEN** one `tasks.task_edited` event names the field `title`, never its value, one `tasks.task_due_changed` event carries the due day before, and sending the same values again tracks nothing

#### Scenario: Handing a task off
- **WHEN** a member hands an open task to a teammate
- **THEN** one `tasks.task_reassigned` event is written with reassignment `handed_off`, whose task has the teammate as `assignee` and the member as `handed_by`, with the member as `previous_assignee`
