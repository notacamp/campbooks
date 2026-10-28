## ADDED Requirements

### Requirement: Send later from the composer
The composer's Send SHALL have a menu with "Tomorrow morning" (tomorrow at
08:00), "Next working day" (08:00 on the next day that falls Monday to Friday) and
"Pick a time…". Each SHALL show the time it resolves to, in the device's
time zone. Choosing one SHALL schedule the draft, close the composer and
show "Scheduled for <time>" with Undo, which cancels the send. ⌘↵ SHALL
still send now.

#### Scenario: On a Friday
- **WHEN** the user opens Send's menu on a Friday
- **THEN** "Next working day" reads Monday at 08:00

#### Scenario: Undo a schedule
- **WHEN** the user schedules a reply for tomorrow and presses Undo in the toast
- **THEN** the send is cancelled and the composer shows the draft again

### Requirement: Scheduled mail in Drafts
Drafts SHALL list sends scheduled more than two minutes ahead in a
"Scheduled" group above the drafts, soonest first. Each row SHALL say when it
leaves and offer "Cancel send", which makes it a draft again. Opening a
scheduled row SHALL offer "Cancel send and edit", which cancels the send and
opens the draft in the composer.

#### Scenario: A scheduled reply
- **WHEN** the user has a reply scheduled for Tuesday at 08:00 and opens Drafts
- **THEN** it shows under Scheduled with "Leaves Tue 29 Oct, 08:00" and Cancel send

#### Scenario: Cancel from the list
- **WHEN** the user presses Cancel send on a scheduled row
- **THEN** it moves to the drafts with "Send cancelled", and nothing is sent
