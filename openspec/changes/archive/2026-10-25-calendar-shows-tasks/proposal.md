# Proposal

## Why

The owner planned tasks for days and none of them showed on the Calendar.
The Calendar already draws Due rows and a task sheet, but nothing feeds them:
`useDueItems` answers an empty list until "the `tasks` follow-up" wires it
(campbooks-cloud#350). And the Due layer as specified shows only a task's
**due** day, while the owner wants the day they **planned** it for as well:
when they mean to do it, next to when it has to be done.

Tracking: https://github.com/notacamp/campbooks-cloud/issues/350

## What Changes

- The Calendar shows the member's own open tasks: on the day each is **due**
  (a Due row, as specified) and on the day it is **planned** (a new Planned
  row, the task's `plan_on`). One row per task per day: when the planned day
  and the due day are the same, only the Due row shows, and it says it is
  planned. A Due row past its day stands on today as "Overdue · N days" (as
  specified); a Planned row past its day also stands on today, as
  Planned, "Was for <weekday>", in the normal colour.
- Agenda, Week (the task lane above All day, now labelled "Tasks") and Month
  all show them. Month did not show Due rows before.
- The task sheet offers Done (Paid for a pay task) and Open task. Hold time
  waits for held time (campbooks-cloud#362) and Later is dropped from the
  calendar's sheet (it hides a task from Home, not from the calendar, so it
  would look like it did nothing); both stay on the task itself.
- The data is the tasks module's own `tasks.list` (the member's open tasks),
  read by the Calendar through the tasks module's public surface, so it is
  live through the change feed like the Tasks place.
- Scout's schedule handbook says planned tasks are on the calendar too, and to
  ask the tasks for them.

Modules touched: `schedule` (client, and its handbook), `tasks` (client: two
more exports), the app shell (wires the conversation sources into the
Calendar's routes as it already does for Tasks). No API, contract, migration
or event change.

## Non-goals

- Held time on the calendar (campbooks-cloud#362).
- Moving a task's plan by dragging it, or from the calendar's sheet.
- Done tasks on past days, and tasks handed to someone else.
- A server-side projection of tasks into `schedule` (see design.md).
