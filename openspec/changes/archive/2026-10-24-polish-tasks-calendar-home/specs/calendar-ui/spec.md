## MODIFIED Requirements

### Requirement: Agenda, week and month
Agenda SHALL list days from today onwards, each with its events in time
order, all-day events first; each event SHALL show its kind's icon and
colour, its time, title and place, and a "from email" mark when Scout read
it from an email. Days with nothing SHALL be skipped, and an empty agenda
SHALL say so in one line. Week SHALL be a time grid with an all-day row and a
line at the current time, events side by side when they overlap. Month SHALL
show each day's first events and "+N more". Cancelled events SHALL not show.
Kind colours SHALL be design tokens. The switch between Agenda, Week and
Month SHALL be compact, sized like the app's other segmented switches (the
small control height, small semibold text, quiet until pressed), with each
view's key shown as a small, quiet key hint; it SHALL never look heavier than
the Calendar's title. From the medium breakpoint the header SHALL keep the
title and period, the switch and the toolbar on one row; on a phone the
switch SHALL take a full-width row of its own under the title.

#### Scenario: A day with a flight and a dinner
- **WHEN** Friday 6 November has Scout's flight 07:40–11:55 and Inês's dinner at 20:00
- **THEN** Agenda shows Friday with the flight (plane icon, "from email") and then the dinner

#### Scenario: Nothing ahead
- **WHEN** the member has no events from today onwards
- **THEN** Agenda says "Nothing on your calendar." in one line

#### Scenario: A quiet switch
- **WHEN** the member opens the Calendar on a desktop
- **THEN** Agenda, Week and Month sit in one compact switch beside the title, each with its key (1, 2, 3) in a small hint, and the header is one row

#### Scenario: The switch on a phone
- **WHEN** the member opens the Calendar at 375px
- **THEN** the switch fills its own row under the title, with nothing scrolling sideways
