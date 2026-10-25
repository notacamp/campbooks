## ADDED Requirements

### Requirement: Invitations awaiting an answer
Home SHALL list each invitation that awaits the user's answer (an attention
item about a `schedule.event`) as a Decide row in its band, read from the
user's calendar: an eyebrow with the kind and the day of the event
("Decide · Event tomorrow", "Decide · Event Thu 29"); the organiser's name,
else address, else the calendar's name, with their avatar; "Invitation:
<title>"; under it the day, the time and the place in the user's zone and
language ("Thursday 29 October, 14:00–15:00 · Office, 3rd floor"); and a
tag saying "You're free then", or "You have <title> then" when another busy,
timed event the user has not declined overlaps it (time held for a task
counts). Selected, it SHALL offer Accept (1), Decline (2) and Maybe (3),
then Snooze (S) and Open in Calendar (Enter), which opens Calendar on the
event's day with the event open; on a calendar that cannot be written it
SHALL offer no answer. An answer SHALL fold the row away with its
confirmation and show a toast ("Accepted: <title>", "Declined: <title>",
"Maybe: <title>") with Undo for 10 seconds, which takes the answer back and
brings the row back. Invitations SHALL count in the hero's "N things need
you" and the Home badge. While an invitation is a Decide row, its event
SHALL not also be Next up, one of today's event rows or a Coming up line.
Without invitations Home SHALL look as before.

#### Scenario: Offsite planning
- **WHEN** João Pereira invited the user to "Offsite planning" on Thursday 29 October, 14:00–15:00, at "Office, 3rd floor", the user has nothing else then, and it is Monday
- **THEN** This week holds "Decide · Event Thu 29", "João Pereira", "Invitation: Offsite planning", "Thursday 29 October, 14:00–15:00 · Office, 3rd floor" and "You're free then"

#### Scenario: Accept with 1, then Undo
- **WHEN** the user selects that row and presses 1, then Undo in the toast
- **THEN** the row folds away with "Accepted: Offsite planning", the toast offers Undo, and Undo brings the row back without João hearing of the answer

#### Scenario: Busy then
- **WHEN** a webinar invitation tomorrow at 10:00 overlaps the user's "Team standup"
- **THEN** its row says "You have Team standup then"

#### Scenario: Read-only calendar
- **WHEN** the invitation is on a calendar the user cannot write
- **THEN** its row offers Snooze and Open in Calendar, and no answer

#### Scenario: Not twice
- **WHEN** an invitation awaiting an answer is today at 11:20 and it is 11:00
- **THEN** it is a Decide row in Today and not Next up; once accepted, it is Next up
