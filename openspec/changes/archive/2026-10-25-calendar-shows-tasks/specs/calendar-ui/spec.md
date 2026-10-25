## MODIFIED Requirements

### Requirement: What is due
The calendar SHALL show the member's own open tasks (the tasks assigned to
them, read from the tasks module's list) on the day each is due and on the
day each is planned.

A task with a due date SHALL show as a Due row on its due day: "Pay · <title>
· <amount>" (the amount only when the task has one, never guessed) or "Do ·
<title>"; "Due 17:00" when due at a time; and, once its day has passed, on
today as "Overdue · 2 days" in the danger colour.

A task planned for a day SHALL show as a Planned row on that day, with the
same title and the word "Planned"; once that day has passed and the task is
still open, it SHALL stand on today as Planned, "Was for <weekday>", in the normal
colour. A task SHALL show at most one row on a day: when its Due row and its
Planned row fall on the same day, only the Due row SHALL show, saying it is
planned. A task put off Later SHALL show no Planned row.

Done and not-needed tasks SHALL drop off, with no strike-through, and a task
handed to a teammate SHALL leave the member's calendar. Changes made
elsewhere (the Tasks place, Home, Scout) SHALL show without a reload.

In Week, tasks SHALL be their own all-day lane above "All day", labelled
"Tasks", Due pills dashed and Planned pills solid; the lane SHALL show only
when the week has tasks. In Month, a day's tasks SHALL be listed after its
events and counted in "+N more".

Opening a task's row SHALL show its sheet: the Due or Planned chip, the
title, the planned day and the due day it has, where it came from ("From
<subject> · <sender>" for a task from an email, opening the email), and the
actions Done (Paid for a pay task, with Undo in a toast) and Open task, which
opens the task in the Tasks place. Once time can be held for a task
(campbooks-cloud#362), a task with time held on its due day SHALL show only
its held block ("Held · <title>").

#### Scenario: A bill due Friday
- **WHEN** Inês has an open pay task "EDP bill" due Friday
- **THEN** Friday shows "Pay · EDP bill" as a Due row

#### Scenario: Planned for Tuesday, due Friday
- **WHEN** Inês planned "Send the signed lease" for Tuesday and it is due Friday
- **THEN** Tuesday shows it as a Planned row and Friday as a Due row, and both open the same task's sheet

#### Scenario: Planned on its due day
- **WHEN** Inês planned "Renew the car insurance" for Thursday, the day it is due
- **THEN** Thursday shows one Due row for it, which says it is planned

#### Scenario: A plan that slipped
- **WHEN** Inês planned "Call the accountant" for yesterday and did not mark it done
- **THEN** today shows "Planned" with yesterday's weekday for it, not in the danger colour

#### Scenario: Overdue
- **WHEN** a do task "Review Marta's proposal" was due two days ago
- **THEN** today shows "Overdue · 2 days" in the danger colour

#### Scenario: Done from the calendar
- **WHEN** Inês opens the "EDP bill" row and presses Paid
- **THEN** the row leaves the calendar, the task is done in the Tasks place, and a toast offers Undo

#### Scenario: Planned in the Tasks place
- **WHEN** Inês plans a task for Wednesday in the Tasks place with the Calendar open in another window
- **THEN** Wednesday shows it as a Planned row without a reload

#### Scenario: Handed off
- **WHEN** Inês hands "Send the signed lease" to Duarte
- **THEN** it leaves Inês's calendar and shows on Duarte's on its days

#### Scenario: Held on its due day
- **WHEN** time can be held for tasks and Inês held 10:00–11:00 on Friday for the task due that Friday
- **THEN** Friday shows "Held · Review Marta's proposal" and no Due row for it
