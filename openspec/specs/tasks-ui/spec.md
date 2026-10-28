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
`n t` (new task, from anywhere in the app), the "New task" button in the
Tasks list's header and the empty state's "Add a task" SHALL open Tasks with
the new-task form in the right pane, at `/w/{id}/tasks?new=1`, in place of a
task's detail; below `lg` it SHALL replace the list, with Back. The form SHALL
take the shape of a task's detail: the title as the page's title ("What needs
doing?", focused), Do or Pay, a "Due" section with the due chips Today,
Tomorrow, the next weekday and Pick a date (which shows the month grid in
place), then "Add task" (Enter) and Cancel (Escape). Adding SHALL create the
task, show "Task added" with Undo, keep the form open and empty with the title
focused for the next one, and show the task in the list. Escape, Cancel,
Close and Back SHALL close the form and give the focus back to "New task";
opening a task SHALL close it. Dates SHALL NOT be read from the typed words.

#### Scenario: Adding for tomorrow
- **WHEN** Ana presses N then T, types "Renew the parking permit", picks Tomorrow and presses Enter
- **THEN** the task appears under Coming up, due tomorrow, and the form stays open with the title empty and focused

#### Scenario: The form in the right pane
- **WHEN** Ana presses "New task" in the list's header
- **THEN** the right pane shows the form instead of a task, the address ends in `?new=1`, and the list stays beside it

#### Scenario: Closing the form
- **WHEN** Ana presses Escape in the form
- **THEN** the form closes, the right pane shows "Choose a task" again and the focus is on "New task"

#### Scenario: A date from the grid
- **WHEN** Ana presses Pick a date in the form and picks 20 October in the month grid
- **THEN** the chip shows that day and the task is created due on 20 October; browsing months changes nothing

#### Scenario: Phone
- **WHEN** Ana opens the form on a 375 px screen
- **THEN** it replaces the list with Back, and nothing scrolls sideways

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
read as the user, under "Why it's here"; an own task with no source SHALL
show only the sentence of that section.

#### Scenario: The lease
- **WHEN** Ana opens the task she made from "Lease for Rua das Flores 12"
- **THEN** the detail shows Ana Ribeiro, that subject, its date and "Open the email"

#### Scenario: Found by Scout
- **WHEN** Ana opens the task Scout made from EDP's invoice
- **THEN** "Why it's here" says Scout made it from an email it read, above EDP's subject and date

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

### Requirement: Hold time
A Do task's detail SHALL offer Hold time (`h`) under "When will you do it?":
the first three free slots of the member's calendar within their working hours,
from now to the end of the due day (or the next 7 days when it has none or it
has passed), with Length (15, 30 or 60 minutes, 30 by default) and Calendar
(the member's writable calendars, the primary provider calendar first when
there is one). Picking a slot SHALL hold it at once with a toast and Undo. A
held task SHALL show "Held <day> <start>–<end>" on its row and in its detail,
where Release SHALL remove the hold with Undo. When there is no free slot the
panel SHALL say so in one line. A Pay task SHALL not offer Hold time.

#### Scenario: Three slots
- **WHEN** Ana presses `h` on "Send the signed lease", due Friday
- **THEN** three free slots before Friday's end show, with Length 30 min and her Google work calendar, and picking "Wed 10:00–10:30" shows "Held Wed 10:00–10:30" with Undo

#### Scenario: A full week
- **WHEN** Ana has no free slot of the chosen length before the task is due
- **THEN** the panel says there is no free time before it is due, and offers the other lengths

#### Scenario: Phone
- **WHEN** Ana holds time on a 375 px screen
- **THEN** the slots, Length and Calendar stack in the detail with nothing scrolling sideways

### Requirement: The Tasks place asks Done? after a reply
A task that asks Done? SHALL show "You replied · Done?" under its title in the
list. Its detail SHALL ask "You replied to <name> at <time>. Is this task done?"
above "Why it's here", with Done (1) and Not yet (2) as large buttons, and
"Why it's here" SHALL list "Your reply · <date>". Both answers SHALL have an
Undo.

#### Scenario: Done from the detail
- **WHEN** Ana opens the task and presses 1
- **THEN** the task is done, with an Undo

### Requirement: Why a task is here, and its history
A task's detail SHALL have a "Why it's here" section whose first sentence says
who made the task and when, by its origin: "Scout made it on <date> at <time>,
from an email it read in your inbox" for a task Scout made, "You made this on
<date> at <time>" for one the member made, "<name> made this on <date> at
<time>" for one a teammate made. A task handed to the member SHALL add "<name>
handed it to you on <date>". Below it a "History" section SHALL list what
happened to the task, oldest first, each entry with its time, a dot by who did
it (a member, Scout or the calendar) and one sentence naming who ("You" for the
member, the teammate's name, "A teammate" when they are not in the member list,
"Scout", "The calendar"). A kind of entry the client does not know SHALL read
"<who> changed it". When there are more than eight entries it SHALL show the
first and the latest six with "Show all <count>" between them, and the button
SHALL show every entry. Both sections SHALL show beside the list and on the
phone detail, without scrolling sideways.

#### Scenario: Made by Scout
- **WHEN** Ana opens the lease task Scout made from Marta's email
- **THEN** "Why it's here" starts "Scout made it on <date> at <time>", and History lists "Scout made it from Marta Ferreira's email, due Fri 16" first

#### Scenario: A long history
- **WHEN** a task has ten entries
- **THEN** seven show, with "Show all 10", and the button shows all ten

#### Scenario: Phone
- **WHEN** Ana opens such a task on a 375 px screen
- **THEN** both sections stack in the detail with nothing scrolling sideways
