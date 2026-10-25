# Design

## Alternatives weighed

1. **Asynchronous, as `add-schedule-module` sketched**: `tasks` publishes
   `TimeHeld{hold_id, task_id, start, end}` and `schedule` makes the event in
   its handler. The member's click would answer before the block exists, the
   picker could not tell a slot was taken in the meantime, and Undo would race
   the job. Two new contracts.
2. **The client makes the event** (`schedule.create_event`) and then plans the
   task: two writes that can half-succeed, and `create_event` only takes an
   email as a source.
3. **Chosen: `tasks.hold` calls a kernel port that `schedule` implements, in
   the same transaction**, as `Availability` and `OpenTasks` already do. The
   event and the task's plan are written together or not at all; the answer
   carries both. Changes made later on the calendar (move, remove, restore,
   provider sync) come back through the `schedule.event_*` events `schedule`
   already publishes, which carry the event's `source`.

## Data ownership

`tasks.tasks` (schema `tasks`) gains three nullable columns, set together or
not at all (a CHECK): `hold_event uuid`, `held_from timestamptz`,
`held_to timestamptz`, and a partial unique index on
`(workspace_id, hold_event) WHERE hold_event IS NOT NULL`. The event itself is
`schedule`'s (kind `hold`, `busy = true`, `origin = member`,
`source = tasks.task:<id>`, title = the task's title, the member's time zone).

## The `Holds` port (kernel, `crates/kernel/src/schedule.rs`)

```rust
pub struct HoldRequest<'a> {
    pub member: UserId,
    pub task: RecordRef,          // tasks.task:<id>
    pub title: &'a str,
    pub starts: DateTime<Utc>,
    pub ends: DateTime<Utc>,
    pub calendar: Option<Uuid>,   // None: the member's Campbooks calendar
}
pub struct Held { pub event: Uuid, pub calendar: Uuid }

pub trait Holds {
    fn hold(&self, conn, workspace, request: HoldRequest) -> HoldFuture;      // Result<Held>
    fn release(&self, conn, workspace, member, event: Uuid) -> ReleaseFuture; // Result<()>
}
```

`NoHolds` (the default) answers `Error::Unavailable`. `schedule` implements it
with the same code as `create_event` and `delete_event` (calendar must be the
member's and writable, else `calendar_read_only` / 404; provider calendars get
their outbound request; `publish::created` / `publish::cancelled` in the same
transaction). `release` of an event already gone or not a hold of that member
is a no-op.

## Endpoints (all `tasks:write`, caller = assignee, task open)

| Endpoint | operationId | Body | Undo |
| --- | --- | --- | --- |
| `POST tasks/{id}/hold` | `tasks.hold` | `revision`, `starts`, `ends` (RFC 3339), `calendar_id?` | `tasks.release_hold` |
| `POST tasks/{id}/release` | `tasks.release_hold` | `revision` | `tasks.hold` (restoring row: the previous `starts`, `ends`, calendar) |

- `tasks.hold`: 400 `invalid_hold` when `ends <= starts`, the length is not
  15–240 minutes, or `starts` is in the past; 400 `pay_not_held` for a `pay`
  task; 409 `task_changed` on a stale revision; 404 when the task is not the
  caller's. An existing hold is released first (moved). Sets
  `plan = date`, `plan_on` = the local day of `starts` in the member's time
  zone (the `TimeZones` port), the hold columns, `revision + 1`, history
  `held` (detail: event id, starts, ends), publishes `tasks.task_changed` with
  `change = planned`.
- `tasks.release_hold`: releases through the port, clears the hold columns,
  `plan = none`, history `released`, `change = planned`. 400 `not_held` when
  there is no hold.
- `tasks.done`, `tasks.not_needed`, `tasks.hand_off`: when the hold has not
  started yet (`held_from > now`), release it in the same transaction and clear
  the columns (history keeps one entry per change, the hold's release is part
  of it). A hold already started or past stays.
- Scope stays `tasks:write`; the app token already has it. Scout's tools do not
  get hold (non-goal), so `.assistant()` is not declared on these two.

`TaskView` gains `hold: { event_id, starts, ends } | null`. The contract
`tasks.task_changed.v1` does not change (`change = planned`, `plan`/`plan_on`
already say it); attention keeps reading it unchanged.

## Following the calendar (tasks consumes schedule events)

A job handler in `tasks` for `schedule.event_created`, `schedule.event_changed`
and `schedule.event_cancelled`, acting only when `source.type == "tasks.task"`:

- **cancelled**: the task whose `hold_event` is this event clears its hold and
  sets `plan = none` (actor `calendar`, history `released`).
- **changed**: updates `held_from`/`held_to` from the event's `starts`/`ends`
  and `plan_on` to the new local day (actor `calendar`, history `held`). An
  all-day or cancelled-status event is treated as cancelled.
- **created** (the calendar's Undo of a removal restores the same event id):
  when the task is open, assigned to the event's owner and has no hold, it
  takes this event as its hold again; else nothing.
- Each is idempotent (compares before writing) and ignores events whose task
  is gone, closed or not the owner's.

`architecture.toml`: the three `schedule.event_*` entries name `tasks` as their
consumer (their "nothing consumes it yet" reasons go).

## Client

- `tasks` exports a `HoldTime` panel and `useHoldSlots`: it calls
  `schedule.free_slots` (`from` = now, `to` = end of the due day or now + 7
  days, `minutes` = the length) and offers the first three starts (the start
  of each free stretch, then later starts within a stretch at the length's
  step, so three choices appear even with one long free afternoon). Calendar
  choices come from `schedule.calendars` (writable ones; the Campbooks calendar
  first, then provider calendars), defaulting to the member's primary provider
  calendar when there is one. The panel takes `tasks` and `schedule` data
  through the app shell's plug (`useFreeSlots`, `useWritableCalendars`) so
  `tasks` does not import `schedule`.
- `useTaskActions` gains `hold(task, {starts, ends, calendarId})` and
  `release(task)`, both with Undo, optimistic like the others.
- Task row, detail and Home show "Held Tue 15:00–15:30" (a chip in the Do
  colour, as in the prototype); the detail's "When will you do it?" gets Hold
  time (`h`) and, when held, Release.
- Calendar (`schedule/due.ts`): a task with a hold has no Planned row; its Due
  row is hidden when the hold's day is the due day. The hold event renders as
  today (kind `hold`); its sheet's "Open task" already leads to the task.
  The task sheet offers Hold time (`h`) for a Do task.

## Tracked events

Two new operations (`tasks.hold`, `tasks.release_hold`) and one new job
handler (`tasks` on `schedule.event_*`) need Coverage rows and tracked events
in the tracking plan (ADR 0020, `docs/tracking.md`): `task_held` and
`task_hold_released` (ids and enums only: task, calendar kind, length in
minutes, actor `member`/`calendar`). Regenerate the plan with the xtask.

## Risks

- A provider push of the hold can fail after the task says Held: the event
  shows the provider error as any member event does, and Release still works.
- Moving the block in Google is only seen at the next sync; until then the
  task shows the old time.
