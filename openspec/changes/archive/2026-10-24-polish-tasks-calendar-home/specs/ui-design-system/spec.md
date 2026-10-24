## ADDED Requirements

### Requirement: Date and time pickers
`lib/ui` SHALL provide `DatePicker` and `TimePicker`, and the app SHALL use
them for every date and time it asks for, never a native date or time input.

`DatePicker` SHALL be a field the height of a text field showing the chosen
day in the member's language ("Mon, Oct 26", with the year when it is not
this year's) or a placeholder, that opens a month grid beside it; or
(`inline`) the grid alone in place. The grid SHALL show the month and year
with previous and next month buttons, the locale's weekday initials starting
on the locale's first day of the week (Monday where the platform does not
say), and six weeks of days: today ringed, the chosen day filled with the
accent, days outside the month muted, days before `min` or after `max`
disabled. It SHALL follow the ARIA grid pattern: each day a button named in
full ("Monday 26 October 2026"), the chosen day's cell `aria-selected`, one
day in the tab order. Arrows SHALL move a day or a week, Page Up and Page Down
a month (with Shift a year), Home and End to the week's start and end, Enter
or Space SHALL pick, and Escape SHALL close and give the focus back to the
field. A day SHALL be sent once, when it is picked, never while months are
browsed. Today and, when `clearable`, Clear SHALL sit under the grid.

`TimePicker` SHALL be an ARIA 1.2 combobox: a text field showing the time in
the member's language (9:30 AM in English, 09:30 in Portuguese, Spanish and
French) and a list of the day's times every `step` minutes (15 by default)
that opens on focus, a click or Down, with the field's time active and in
view. Up and Down SHALL move the active time, Enter SHALL take it, Escape
SHALL close the list and nothing else. A typed time SHALL be taken on Enter
or on leaving the field when it reads as a time ("9", "930", "9:30", "21:15",
"9.30", "21h15", "9pm", "9:30 am"); anything else SHALL go back to the last
time. With `min`, earlier times SHALL not be listed and an earlier typed time
SHALL not be taken.

Both SHALL carry a label, help and an error like the other fields, and their
own words (Previous month, Next month, Today, Clear, Choose a date, Choose a
time) in English, European Portuguese, Spanish from Spain and French.

#### Scenario: A day from the keyboard
- **WHEN** Ana opens a date field on 26 October, presses Right, Down and Enter
- **THEN** the grid moves to 27 October, then 3 November (showing November), and the field takes 3 November once, closing with the focus on it

#### Scenario: Nothing in the past
- **WHEN** a date field has `min` today and Ana opens it
- **THEN** yesterday is disabled, the previous month cannot be shown, and the arrow keys stop at today

#### Scenario: A typed time
- **WHEN** Ana types "2115" in a time field and leaves it
- **THEN** the field takes 21:15 and shows it as "9:15 PM" in English, "21:15" in Portuguese

#### Scenario: An end after its start
- **WHEN** an event starts at 10:00 and Ana opens its end time
- **THEN** the list starts at 10:00, and typing "9:00" leaves the end as it was

#### Scenario: Escape in a dialog
- **WHEN** Ana presses Escape with a time list open in the new event dialog
- **THEN** the list closes and the dialog stays open
