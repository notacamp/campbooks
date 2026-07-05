# Tasks and calendar

## Tasks (feature-gated)

Tasks are available when the workspace has the tasks feature enabled. All task
tools check this automatically and return a ToolError if the feature or
entitlement is off.

### Statuses

| Status | Meaning |
|---|---|
| suggested | AI-extracted from an email or document; not yet confirmed |
| todo | Active, not started |
| in_progress | Actively being worked on |
| blocked | Waiting on something external |
| done | Completed (stamped completed_at) |
| cancelled | Cancelled (soft; not deleted) |

Suggested tasks appear in get_overview as `tasks.suggested_count`. Confirm one
by calling update_task(id, status: "todo") — this runs the proper status
transition and publishes domain events.

### Create from email

create_task_from_email(email_id) uses the action registry to extract a task
from the email's content. Pass `title` to override the extracted title. The
result is a suggested task — ask the user to confirm before moving it to todo.

### Completing tasks

complete_task(id) calls the proper move_to_status!(:done) transition, which
stamps completed_at and publishes events. Do not use update_task for completion;
use complete_task.

## Calendar

### Calendars

list_calendars returns all calendars visible to the user. The `writable` field
indicates whether the user can create/update events on that calendar. Use the
`id` as `calendar_id` when creating events.

### Events CRUD

- list_calendar_events — accepts `start_after` and `start_before` for windowed
  fetches. Default order is soonest first.
- get_calendar_event(id) — full detail including attendees and RSVP.
- create_calendar_event — requires calendar_id, title, start_at. end_at defaults
  to one hour after start if omitted by the server.
- update_calendar_event — `recurrence_scope: "this"` (default) updates only this
  occurrence; `"all"` updates the whole series.
- delete_calendar_event — async provider delete. The event is soft-deleted
  locally before the provider confirms.
- rsvp_calendar_event — sets your attendance status on an event you have been
  invited to.

### Create from email

create_event_from_email(email_id) extracts event details (title, time, location)
from the email's content using the AI extractor. Returns the created event.
Pass overrides (title, start_time, end_time, calendar_id) to refine the
extracted values. Always show the extracted event to the user before creating it.

## Reminders

AI-extracted reminders surface dated commitments found in emails and documents.

- confirm_reminder(id) converts the reminder into a calendar event on the user's
  primary writable calendar. Pass `due_at` to adjust the time first.
- dismiss_reminder(id) marks it dismissed without creating an event.
- snooze_reminder(id, until:) postpones it. Defaults to one week if `until` is
  omitted.

get_overview shows `reminders.pending_count` and `reminders.overdue_count`.
