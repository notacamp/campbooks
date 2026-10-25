# Design

## Where the data comes from

`add-schedule-module` planned the Due layer as a projection: `schedule`
consumes `tasks.task_changed.v1`, keeps the latest revision per task in its
own table, and reads titles and visibility through a `TaskSource` port. Two
ways were weighed:

1. **Projection in `schedule`** (the plan). A new table, a consumer that
   orders by revision and follows hand-offs (`previous_assignee`), a
   `TaskSource` port for the title and who may see it, and a
   `schedule.list_due` endpoint. It duplicates what `tasks.list` already
   answers and puts a second copy of "who sees a task" in another module.
2. **The Calendar reads `tasks.list`** (chosen). The member's open tasks
   (`state=open`, `whose=mine`) are already one call, already scoped to the
   assignee by the tasks module's own access rule ("Nobody else sees a task"),
   already refetched by the change feed when a `tasks.task` changes, and
   already cached for the Tasks place and Home. The Calendar filters them to
   the days it shows. No new table, endpoint, scope, event or port.

The cost of (2) is the list's cap: `tasks.list` answers at most 500 open
tasks, ordered overdue first, then by due day, then undated. A member with
more than 500 open tasks could miss the oldest planned-but-undated ones. That
is far past any real list; if it ever matters, `tasks.list` gains a day window
(`due_on` or `plan_on` between two days) without changing the Calendar.

Data ownership is unchanged: tasks live in `tasks.tasks` (the `tasks`
schema); `schedule` stores nothing about them. Scope: `tasks:read`, which the
app's token already holds for the Tasks place.

`tasks.task_changed.v1` keeps its description ("the calendar's Due layer will
read it") until a server-side reader needs it; this change does not consume
it.

## Client seams

- `~/modules/tasks` exports `useTasks` (the open list, unchanged) next to the
  `useTaskActions` it already exports; `schedule` imports them from the
  module's public surface only, as `attention` and `conversations` already do.
  `tasks` does not import `schedule`, so there is no cycle.
- The sender and subject of a task from an email come from the conversations
  module's `useConversationSources`, plugged into `buildScheduleRoutes` by the
  app shell exactly as it is plugged into `buildTasksRoutes` (`schedule`
  cannot import `conversations`, which imports `schedule`).
- `schedule/due.ts` stops being a cache-only stub: its items are derived from
  the tasks list, and its actions call the tasks module's Done (with the
  tasks module's own Undo toast). The `DUE_ACTION_KEY` mutation-defaults seam
  goes away.

## What shows where

For each open task assigned to the member, with today being the member's day
in the Calendar's time zone:

- **Due row** when it has a `due_on`: on that day, or on today once that day
  has passed ("Overdue · N days", danger colour). "Due 17:00" when it has a
  `due_at`.
- **Planned row** when `plan = date` and it has a `plan_on`: on that day, or on
  today once it has passed (Planned, "Was for <weekday>", normal colour), unless the
  Due row stands on the same day, in which case the Due row shows alone and
  its second line says "Planned".
- A Later plan shows no Planned row; its Due row still shows.
- Agenda orders a day's rows: overdue, all-day events, Due and Planned rows
  without a time, then timed rows. Week puts both in the task lane (Due
  dashed, Planned solid). Month counts them with the day's events toward
  "+N more", events first.

## Tracked events

None added or changed: no new operation, job, schedule or handler. The
calls are the existing `tasks.list` and `tasks.status`, already covered.

## Risks

- Two rows for one task in a week (planned Tuesday, due Friday) can read as
  two tasks. Both rows carry the same title and their own word (Planned,
  Due), and open the same sheet.
