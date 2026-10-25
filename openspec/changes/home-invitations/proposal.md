# Proposal

## Why

The owner asked Home to order what needs the member by priority, using the
tasks and the calendar. Since campbooks-cloud#358 Home orders by bands, and
since campbooks-cloud#359 it shows the member's day. But an invitation the
member has not answered is only in Calendar: "Offsite planning, Thursday
14:00" waits there until the member happens to open the event, and the
organiser waits for an answer.

This change makes every invitation that awaits the member's answer a Decide
row on Home, in its band by the day of the event ("Decide · Event Thu 29"),
with Accept (1), Decline (2) and Maybe (3) on the row, "You're free then" or
"You have <title> then", and Undo in the toast. It counts in the Home badge
and "N things need you", and Scout's `what_needs_me` lists it with what it
is and who asked.

The visual reference is the approved prototype, options "Priority 2 · Rows B
· Layout II · Phone stacked", item `q2`.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/360

Modules touched: `schedule` (keeps an event's organiser, announces
invitations on a new event `schedule.invitation_changed` v1, an answer's
undo window, a one-off backfill, the container `schedule.member`),
`attention` (projects invitations into Decide items, bands them by the day
of the event), `assistant` (`what_needs_me` names the invitation), the
contracts (`contracts/schedule.invitation_changed.v1.schema.json`), and on
the client `modules/schedule` (the invitation rows in words, their answers
with Undo, the free/busy fact) and `modules/attention` (Home places them).
No new module and no new cross-module dependency: `schedule` now depends on
`campbooks-ai` for the facts strategy's machine-sender test, which every
module may (AGENTS.md); the dispatcher's maintenance scope has its
`architecture.toml` entry.

## What Changes

- **Invitations are announced.** An event of one of the member's provider
  calendars that syncs, which the member was asked to (`needs_action`) and
  did not organise, a single event or a series, not cancelled, deleted or
  started, is announced as `schedule.invitation_changed` v1 with a revision:
  the day it starts in the member's zone, when it stops awaiting an answer
  (its start; an all-day one, the end of its first day) and whether a
  person organises it (the organiser's address alone). An answer, a cancel,
  a delete, the calendar turned off or removed announce it closed. The
  provider's organiser is now kept with the event and shown on it.
- **Decide items.** `attention.project_invitation` keeps one Decide item per
  invitation in the member's own container, idempotent and order-proof.
  Its band follows the day of the event: today or tomorrow is Today ("Event
  today", "Event tomorrow"), within 7 days This week, later Whenever ("Event
  Thu 29"); it is never overdue and never "waiting", and it leaves the list
  when its event starts.
- **Answers with Undo.** Accept, Decline and Maybe use the existing
  `schedule.respond_event`. On a provider calendar the answer now reaches the
  provider only once a 10-second undo window has passed, as a delete does:
  within it a new answer replaces it and "not answered" takes it back, so
  Undo never reaches the organiser.
- **Home.** The Decide row: the eyebrow, the organiser's name, "Invitation:
  <title>", "<day>, 14:00–15:00 · <place>", the free/busy tag, Accept (1),
  Decline (2), Maybe (3), Snooze (S) and Open in Calendar (Enter); on a
  calendar that cannot be written, no answers. An invitation shows as the
  Decide row only, never also as a calendar row.
- **Scout.** `what_needs_me` lists an invitation with its event's line and
  who asked.
- **Backfill.** Invitations stored before this change are announced once by
  a minute dispatcher, from the schedule module's own rows: no resync, no
  model.

## Non-goals

- Ranking by meetings with the sender, and Scout's urgency as an input
  (campbooks-cloud#361).
- Answering one occurrence of a series apart from the series.
- Telling the provider "not answered" once it has an answer (no provider
  takes an answer back).
- Hold time from Home (shipped in campbooks-cloud#362).
