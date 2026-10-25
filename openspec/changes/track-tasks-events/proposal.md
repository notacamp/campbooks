# Track the tasks module's changes

Module: `tasks` (`crates/modules/tasks/`: its operations, its job and its
event handlers), with the tracking plan (`tracking/`). Tracking:
https://github.com/notacamp/campbooks-cloud/issues/340, part of epic
https://github.com/notacamp/campbooks-cloud/issues/319.

## Why

ADR 0020 asks for a record of every action: who did it, in what context,
and what the records it touched looked like then. The tasks module holds
what each member has to do or pay: the tasks they write, the asks Scout
reads out of their mail, when they plan to do them, what they close and
what they hand to a teammate. It landed before the tracking plan
(notacamp/campbooks#628), so none of it is tracked and the record cannot
answer "how many of Scout's asks get done", "how often is a task handed
off, and taken back" or "how far ahead do members plan". Full coverage
becomes a CI failure with campbooks-cloud#332, so every operation, job and
handler of the module needs a Coverage row first.

The attention module already has its row for the projection of
`tasks.task_changed` (`attention.project_task`); the tasks events are the
record of the changes themselves.

## What Changes

- Tracking plan version 10 (`tracking/patches/0010-tasks.yaml`): 9 new
  live events of `tasks`: `task_created`, `task_edited` (which fields, by
  name), `task_due_changed` (with the due date before), `task_planned`
  (with the plan before), `task_done`, `task_not_needed`, `task_reopened`
  (with the state before), `task_handed_off` and `task_taken_back` (with
  the member who had it).
- The records `tasks.task`, `tasks.task_due` and `tasks.task_plan`, their
  enums, and a Coverage row for each of the module's 8 mutating
  operations, its job and its 2 event handlers.
- Each change is tracked with `Tracker::track` in the transaction that
  makes it, with a snapshot read in that transaction, from the one place
  every change of a task ends (`domain::finish`). A change that changes
  nothing tracks nothing, including a plan set again to what it was.
- Tests that read every event back from `kernel.tracked_events`.

No new endpoint, scope, migration, dependency or domain event. No change to
what any endpoint returns, to the task's history or to
`tasks.task_changed`.

## Non-goals

- The other modules and the hard coverage gate (#332).
- Reads (`tasks.list`, `tasks.get`) and the port `open_tasks`: they change
  nothing.
- Any personal data or content: no title, sender address, amount or
  currency, quote from the email, or a person's name. User ids are
  pseudonymous; whether a task has an amount is a boolean.
