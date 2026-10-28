## MODIFIED Requirements

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

### Requirement: The Tasks place asks Done? after a reply
A task that asks Done? SHALL show "You replied · Done?" under its title in the
list. Its detail SHALL ask "You replied to <name> at <time>. Is this task done?"
above "Why it's here", with Done (1) and Not yet (2) as large buttons, and
"Why it's here" SHALL list "Your reply · <date>". Both answers SHALL have an
Undo.

#### Scenario: Done from the detail
- **WHEN** Ana opens the task and presses 1
- **THEN** the task is done, with an Undo

## ADDED Requirements

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
