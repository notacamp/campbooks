# Proposal

## Why

The owner wants to tell the calendars that are theirs from the ones they only
follow, and to know which calendars feed their tasks.

Today every calendar that is on counts as the member's own:

- **Free time:** its events make the member busy. Free time is what Hold time
  and Scout's free-time answers use.
- **Holds:** time can be held for a task on any calendar the member can write to.
- **Invitations:** a pending invitation on it becomes a Decide need on Home.

The connectors read who owns a calendar (Google's `accessRole`, Zoho's
`privilege`) and keep only "may be written", so Campbooks cannot tell "my
calendar" from "a colleague's calendar shared with me that I can edit". That
also causes a real bug. Google marks as `self` the attendee *whose calendar it
is*. A colleague's shared calendar that is turned on therefore brings the
colleague's unanswered invitations onto the member's Home as their own Decide
needs.

Tracking: https://github.com/notacamp/campbooks-cloud/issues/399

## What Changes

- Every calendar has a **role**: **Mine**, **Following** or **Off**.
  - **Mine:** shown; it makes the member busy for free time and Hold time; a
    task's time can be held on it (when it can be written); its invitations
    go to Home.
  - **Following:** shown only.
  - **Off:** hidden, as a calendar with sync off is today.
  - The Campbooks calendar is always Mine.
- Connectors add whether the account **owns** each calendar to the calendar
  list it reads at grant time. This is an optional field in `calendar-list` v1:
  Google `accessRole = owner`, Zoho `privilege = owner`.
- A new provider calendar starts **Mine** when the account owns it and
  **Following** otherwise. Only the primary calendar is on at first, as today.
  The member can change any role in Settings → Calendar.
- Calendars that exist already keep their behaviour. One that syncs stays
  Mine, except a calendar that cannot be written, which becomes Following.
  A repeated grant (reconnecting, adding calendar access again) records the
  ownership fact on calendars that lack it, without changing their role.
- API: `CalendarView` gains `role` (`mine` | `following` | `off`) and
  `owned` (`true` | `false` | `null` when unknown). The calendar PATCH accepts
  `role`. `syncing` stays, meaning "not off".
- Free slots count only Mine calendars. A hold on a Following calendar is
  refused as `calendar_followed`. An invitation awaits an answer only on a Mine
  calendar. Making a calendar Following or Off closes its open invitations as
  `hidden`, and making it Mine again re-announces them.
- Settings → Calendar groups calendars under Mine, Following and Off, with a
  three-way control on each provider calendar. The calendar pickers for Hold
  time, "Add to my calendar" and "Also add to" offer only writable Mine
  calendars.
- Scout's schedule handbook explains the roles, and says free time counts only
  the member's own calendars.

Modules touched: `connectors` (calendar list), `schedule` (migration, role,
free time, holds, invitations, API, handbook), `calendar-ui` (client).
Contract: `calendar-list` v1 gains an optional field. Nothing in `attention`
changes: it already closes an invitation announced `hidden`.

## Non-goals

- Re-reading the calendar list on a timer to notice a calendar shared with
  the member later. It is read when calendar access is granted, as today.
- Tasks made from calendar events. Obligations stay tasks from mail, as the
  handbook says.
- Showing a Following calendar differently on the Calendar views (for example
  muted). Its colour already sets it apart. This can follow if wanted.
