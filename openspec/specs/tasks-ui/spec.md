# tasks-ui Specification

## Purpose
The Tasks place of the app: the member's open tasks by when they are due, quick add, a task's detail with its plan, source and hand-off, and the keys and undo that go with them.

## Requirements

### Requirement: The Tasks place
The app SHALL have a Tasks place at `/w/{id}/tasks`, reached from the sidebar
and with `g t`, that shows the open count in the nav and lists the user's open
tasks in the groups Overdue, Today, Coming up and No date yet, leaving out
empty groups, plus a folded Later group with its count.

#### Scenario: Ana's Friday
- **WHEN** Ana has an overdue bill, two tasks for today, two coming up, one undated and one put off
- **THEN** Tasks shows Overdue (1), Today (2), Coming up (2), No date yet (1) and a folded Later (1)

#### Scenario: Nothing to do
- **WHEN** Ana has no open task
- **THEN** Tasks shows an empty state that offers "Add a task" with its key

### Requirement: A task row
Each task row SHALL show a round checkbox, the verb as a Do or Pay tag, the
title on one line, where it came from ("From <sender> · email" or "You wrote
this"), and chips for the due date (overdue in the danger colour) and the plan.

#### Scenario: An overdue bill from EDP
- **WHEN** Ana's EDP bill task was due two days ago
- **THEN** its row shows a Pay tag, "From EDP · email" and "Overdue · 2 days" in the danger colour

### Requirement: Quick add
`n t` (new task, from anywhere in the app) SHALL open Tasks and focus "Add a task…". Typing SHALL offer the due chips Today,
Tomorrow, the next weekday and Pick a date, and a Do/Pay choice. Enter SHALL
create the task, keep the field focused for the next one, and show "Task added"
with Undo. Dates SHALL NOT be read from the typed words.

#### Scenario: Adding for tomorrow
- **WHEN** Ana types "Renew the parking permit", picks Tomorrow and presses Enter
- **THEN** the task appears under Coming up, due tomorrow, and the field is empty and focused

### Requirement: A task's detail
Opening a task (Enter or a click) SHALL show its title (editable), verb, due
date with Change, "When will you do it?" with Pick a day and Later (Tonight,
Tomorrow, Next week, Pick a date), "Hand it to someone" as a filter box by
teammate name, and the actions Done (or Paid with "Campbooks never pays for
you — this just marks it paid.") and Not needed. At `lg` and up the detail
SHALL sit beside the list; below `lg` it SHALL replace the list with a Back
button.

#### Scenario: A bill
- **WHEN** Ana opens "EDP electricity bill"
- **THEN** the primary action reads "Paid" with its key, and the note says Campbooks never pays for her

#### Scenario: Phone
- **WHEN** Ana opens a task on a 375 px screen
- **THEN** the detail fills the screen with Back, and nothing scrolls sideways

### Requirement: Where a task came from
A task's detail SHALL show, for a task with a source conversation, the
sender, subject and date of that conversation and an "Open the email" link,
read as the user, and for a task of origin `ask` also that Scout found it in
that email; an own task with no source SHALL show nothing there.

#### Scenario: The lease
- **WHEN** Ana opens the task she made from "Lease for Rua das Flores 12"
- **THEN** the detail shows Ana Ribeiro, that subject, its date and "Open the email"

#### Scenario: Found by Scout
- **WHEN** Ana opens the task Scout made from EDP's invoice
- **THEN** the detail says Scout found it in that email, above EDP's subject and date

### Requirement: Task keys and undo
In the Tasks place `n t` SHALL add, `j`/`k` SHALL move, Enter SHALL open, `e` SHALL mark done or
paid, `s` SHALL offer Later, `x` SHALL mark not needed, Esc SHALL close the
detail, and `z` SHALL undo the last change. Every change SHALL take effect at
once, show a toast with Undo, roll back and say so if the server refuses, and
the shortcuts SHALL show as key hints and in the `?` sheet.

#### Scenario: Done by key
- **WHEN** Ana selects a task and presses `e`
- **THEN** it leaves the list, "Done" shows with Undo, and `z` brings it back

### Requirement: Handed-off tasks
A task the user handed to a teammate SHALL show under its group with "With
<name>" and Take it back, and taking it back SHALL make the user its assignee
again.

#### Scenario: Taking it back
- **WHEN** Ana takes back the task she handed to João
- **THEN** it is Ana's again and leaves João's Home

### Requirement: Ask Scout about a task
A task's detail SHALL offer "Ask Scout" in its header row, with the key
`.` shown on it, wherever a chat can be opened. It SHALL open Scout's chat
about that task, and the focus SHALL return to where it was asked from when
the chat closes. The chat SHALL name the task by its title and offer
starter questions about what is left, when to do it and who could take it.

#### Scenario: Asking about the lease
- **WHEN** Ana opens "Send the signed lease to Marta" and presses `.`
- **THEN** Scout's chat opens about that task, naming it and offering "What's left on this?", "When should I do this?" and "Who could take this?"
