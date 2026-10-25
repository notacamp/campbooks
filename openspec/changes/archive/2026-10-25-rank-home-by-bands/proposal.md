# Proposal

## Why

Home groups what needs the member by reason: Review, Reply, Decide, Do, Pay,
Nudge, Worth a look. A bill overdue by a week sits under three replies that
can wait, and a person who has waited four days for an answer looks the same
as one who wrote an hour ago. The owner asked to "order items in priority
order independently of their type".

This change orders Home by **priority bands**: Now, Today, This week and
Whenever, worked out from facts with simple rules that are easy to predict
and to explain. Each row says first what it is and why it is where it is
("Pay · Overdue · 2 days", "Reply · Marta has waited 4 days"), and the
opened item says which band it is in and why. Home also stops listing every
task: only the ones for today, with a line saying how many more are in Tasks.

The visual reference is the approved prototype, options "Priority 2 · Bands"
and "Rows B · Why first".

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/358

Modules touched: `attention` (the bands, the list, the projections and its
handbook), `conversations` (two facts on `need_changed`), `assistant`
(`what_needs_me` follows the bands), and on the client `modules/attention`
(Home) and `lib/ui` (`Item` gains an eyebrow line, `QueueGroup` a group
without a signal). No new module, no new cross-module dependency, no new
crate.

## What Changes

- **Bands.** `attention.list` (open) puts each item in one band by the first
  rule it meets: a scam warning or anything overdue is Now; due today or
  tomorrow, a task planned for today, a nudge, or a person who has waited 3
  days or more for a reply is Today; due in the next 7 days, or a person
  waiting for a reply or a choice, is This week; the rest (no date, from
  services, kept on Home by the member's rule) is Whenever. Inside a band:
  warnings first, dated before undated and the soonest date first, people
  before services, the oldest first. Each item carries its `band` and its
  `band_reason` (the rule, with the days or the day it names).
- **Only today's tasks on Home.** A task item is listed only when it is
  overdue, due today or tomorrow, planned for today or a day before, or back
  from Later; the response says how many others were left out
  (`tasks_elsewhere`).
- **Facts.** `conversations.need_changed` v1 gains two optional fields on the
  need: `warning` (the suspicious-mail warning) and `from_person` (a person
  sent the message, from the headers alone). `attention` keeps them, and a
  task's plan and planned day from `tasks.task_changed`. Open needs are
  announced again once with the new facts, by the minute clock, with no
  model asked.
- **Scout.** `what_needs_me` lists needs in the same order, each with its
  band and why, instead of Scout's own urgency, and says how many tasks are
  only in Tasks.
- **Home.** The groups are the bands: Now (in the danger tone), Today, This
  week, Whenever, each with its count. Each row has an eyebrow line before
  the name: the kind's signal dot and name, then the reason in words, in the
  danger tone for a warning or overdue; the row's end shows only the age.
  The opened item gains "Why it's here". The Pay and Nudge notes move into
  the opened item of those kinds; "All N already paid" shows once, under the
  list; "N more in Tasks" with Open Tasks (`G` `T`) ends the list.

## Non-goals

- Scout's urgency as an input to the bands: its precision is below the
  floor (campbooks-cloud#361).
- Meetings with the sender as a fact: the calendar has no attendees yet
  (campbooks-cloud#361).
- Calendar rows, Next up and Coming up on Home (campbooks-cloud#359).
- Invitations as their own kind (campbooks-cloud#360).
- Hold time from Home (campbooks-cloud#350).
- Learning the order from what the member does ("Bands that learn").
