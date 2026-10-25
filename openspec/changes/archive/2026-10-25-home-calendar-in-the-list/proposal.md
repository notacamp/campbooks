# Proposal

## Why

Home says what needs the member, ordered by priority bands since
campbooks-cloud#358, but it knows nothing of their day: a call in twenty
minutes, the time they held for a task or the parcel arriving tomorrow are
only in Calendar. The owner asked for Home to use the calendar and the tasks,
so that one screen answers "what now, and what is next".

This change puts the member's calendar in Home's list, as the approved
prototype draws it ("Priority 2 · Rows B · Layout II · Phone stacked"): the
next event above Now, today's events and held time at the end of Today, the
hero saying what is next, and a "Coming up" list of the week in the aside.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/359

Modules touched, all on the client: `modules/attention` (Home places the
rows), `modules/schedule` (reads the day for Home and exports it through its
public surface: `useHomeDay`), and `lib/ui` (`Item` draws an event's kind in
place of an avatar, `KindDot`, a `QueueGroup` without a count). No server
change, no new module, no new cross-module dependency: `attention` already
imports other modules' public surfaces (`conversations`, `tasks`) and now
`schedule`'s.

## What Changes

- **Next up.** Above Now, a group "Next up" holds the member's next event
  when it starts within two hours (not all-day, not held time, not declined
  or cancelled), until ten minutes after it started. Its row has the kind's
  square instead of an avatar, "Meeting · In 20 min" before the title, the
  place (or the calendar's name) under it and its time at the end; selected,
  it offers Join (only when its place is an `https://` address), Open email
  (when it was read from one) and Open in Calendar (Enter).
- **Today's calendar in the list.** The rest of today's events and held
  time, after now, close the Today band in start order ("Reservation ·
  12:30", "Held · 15:00", all-day ones "All day"), Today showing for them
  alone when no item is in it. They take part in ↑/↓; Enter opens one in
  Calendar on its day. They never count in the hero's "N things need you",
  the Home badge or Today's count. The hero's line becomes the day's facts:
  "Next: <title> at <time>. N more on your calendar today."
- **Coming up.** First in the aside, up to six dated things from tomorrow
  to a week after it, soonest first: events of any kind and the tasks due
  (the calendar's Due layer). Each line has its short day, a dot in its
  kind's colour (a ring when only the member sees it), the title and its
  time, or "Add to my calendar" (the existing push, with its Undo) for an
  event only the member sees that one of their calendars can take; then
  Open Calendar (`G` `S`). Hidden when the week is empty.

## Non-goals

- Invitations as Decide rows on Home (campbooks-cloud#360).
- Hold time from Home, and meetings ranking items (campbooks-cloud#361).
- A time grid of the day on Home (the prototype's Layout I and III).
- Any server change: the schedule API already lists the window, the
  calendars and settings, and pushes an event.
