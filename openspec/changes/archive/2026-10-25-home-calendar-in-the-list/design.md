# Design

## Data ownership

- **schedule** owns the events, the calendars, the member's zone and the
  Due layer (`schedule` schema, unchanged). Home reads them only through
  `schedule`'s public client surface: `useHomeDay(workspaceId, { now,
  selected })` returns Next up, the rest of today and Coming up as rows in
  words, each with what it can do (Join, Open in Calendar, Open email, Add to
  my calendar). It is built from the calls the Calendar already makes:
  `schedule.get_settings` (the zone), `schedule.list_calendars`,
  `schedule.list_events` over today and the 7 days after it (one window, the
  server's filter: the member's calendars with `syncing` on and their
  Campbooks calendar), `tasks.list` through the Due layer (`useTaskRows`),
  `conversations.get` for the selected event's email (as the event sheet
  does), and `schedule.push_event` for Add to my calendar (`useEventActions`,
  its toast and Undo).
- **attention** owns nothing new. Home places the rows: Next up as a group
  above Now, today's at the end of Today (a Today of events alone is headed
  by its name, no count), Coming up first in the aside.
- The pure part is `schedule/day.ts`: `nextUp`, `laterToday`, `comingUp`,
  `offersAdd`, `pushTarget`, `joinUrl`, `dayFacts`, over the member's wall
  clock (`nowIn` in their calendar's zone), the same the Calendar draws its
  now line with. It re-evaluates each minute (`useClock`, now shared with the
  Calendar screen), so Next up leaves ten minutes after its start.

No server change was needed, so no endpoint, scope, migration, event or job
is added.

## Rules

| What | Rule |
|---|---|
| Next up | earliest timed event, not `hold`, not `cancelled`, not `declined`, starting in at most 120 minutes or less than 10 minutes ago |
| Rest of today | not cancelled or declined, not Next up; timed events and holds starting from now, all-day events covering today; all-day first, then by start |
| Coming up | tomorrow to tomorrow + 6 days (7 days), events of any kind and Due rows (not Planned rows), by day, then all-day/untimed, then time; at most 6; only when the window has an event |
| Add to my calendar | not `hold`, on the member's Campbooks calendar, not on a provider (`canPush`); to the calendar added to last, else the primary writable one, else the first writable |
| Join | `place` parses as an `https:` URL (a structured field, not the email's words) |
| Hero facts | the first timed, non-held row of today (Next up or later), and how many more rows today has |

Time zone: everything on the calendar side uses the member's zone
(`effective_time_zone`), so Next up's countdown and today's boundary match
the Calendar screen. The bands still use the device's offset, as before.

## AI

None. Every row is a fact from the calendar or the tasks; nothing reads an
email's words. Nothing is changed without the member: Add to my calendar is
their own press, with Undo in the toast.

## Scopes

Unchanged; the calls are the Calendar's own (`schedule:read`,
`schedule:write` for the push, `tasks:read`, `conversations:read`).

## Tracking coverage

Checked `tracking/tracking-plan.xlsx` (Coverage) and `docs/tracking.md`: no
operation, job, schedule or handler is added. Opening an event in Calendar,
Join and Open email are navigations, which the plan does not track. Add to
my calendar calls `schedule.push_event`, whose Coverage row and tracked
event already exist. Tracking plan unchanged.
