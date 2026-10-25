# Proposal

## Why

The tasks prototype the owner approved (campbooks-cloud#43) has **Hold time**:
the member picks one of three free slots from their calendar, a length and a
calendar, and the task shows "Held 15:00–15:30" while the calendar shows the
block. It was left as a follow-up of the calendar (#42) and of
`calendar-shows-tasks` (#350), which shows tasks on their due and planned
days but has no held time.

Tracking: https://github.com/notacamp/campbooks-cloud/issues/362

## What Changes

- **Hold time on a task.** In the task's detail (`h`), on a Do task on Home
  (`h`) and in the calendar's task sheet (`h`), Hold time offers the first three
  free slots of the member's calendar within their working hours (from now
  until the end of the due day, or the next 7 days when there is none or it has
  passed), with Length (15, 30 or 60 minutes, 30 by default) and Calendar (the
  member's writable calendars). Picking a slot holds it.
- **A hold is an event and a plan.** Holding makes an event of kind `hold` on
  the chosen calendar (titled with the task's title, busy, linked to the task)
  and plans the task for that day. A provider calendar receives it like any
  event the member makes; due dates still never go to a provider. One hold per
  task: holding again moves it.
- **Release.** "Release" on the task (and Undo of a hold) removes the block and
  sets the plan back to none. Removing the block on the calendar does the same;
  moving it (in Campbooks or in Google) moves the task's plan with it.
- **Done, not needed, handed off.** A hold still ahead is released when the
  task is done, not needed or handed to someone (it was on the member's own
  calendar); a hold already past stays as a record.
- **What shows.** The task shows a "Held Tue 15:00–15:30" chip (rows, detail,
  Home). On the calendar the held block replaces the task's Planned row, and on
  the due day it replaces the Due row too, as `calendar-ui` already says.

Modules touched: `tasks` (endpoints, columns, consumer of schedule events,
client), `schedule` (implements a new kernel port, client), `kernel` (the
`Holds` port), `attention`/Home client (the Hold action). New cross-module
dependency: `tasks` → `schedule` through the kernel `Holds` port (like
`Availability`, `OpenTasks`), and `tasks` consumes `schedule.event_created`,
`schedule.event_changed` and `schedule.event_cancelled` (listed in
`architecture.toml`).

## Non-goals

- Scout holding time on its own, or proposing holds in chat.
- More than one hold per task, or holds for Pay tasks (Home and the Pay
  detail offer no Hold; a bill is paid, not worked on).
- A drag-to-hold on the calendar grid.
- Holds for shared inboxes' tasks of other members.
