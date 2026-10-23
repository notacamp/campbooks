## ADDED Requirements

### Requirement: Tasks in Do and Pay
Home's Do and Pay groups SHALL include the user's open task items, rendered
from `tasks.list` with `ids`, beside conversation items and in the same order
by due date. A task row SHALL show its title, where it came from and its due
chip. Its actions SHALL be Done (Paid for `pay`) `e`, Later `s` and Not needed
`x`, calling `tasks.done`, `tasks.plan` with `later` and `tasks.not_needed`,
each with Undo.

#### Scenario: The lease and the bill
- **WHEN** Ana has a task "Send the signed lease to Ana" due today and an overdue EDP bill task
- **THEN** Do lists the lease task and Pay lists the bill, and pressing `e` on the bill marks it paid with Undo

#### Scenario: Later from Home
- **WHEN** Ana presses `s` on a task and picks Tomorrow
- **THEN** the task leaves Home until tomorrow and Tasks shows it under Later
