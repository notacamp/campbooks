## Purpose

The Calendar screens of the app: the Calendar place with its agenda, week and
month views, the event sheet, the strip in the reader that says what Scout
found in an email, and Settings → Calendar.

## ADDED Requirements

### Requirement: The Calendar place
Calendar SHALL be a place next to Home and Inbox in the sidebar and in the
phone tab bar, opened with G then S (`nav.schedule`, already in the shortcut catalogue). It SHALL open on Agenda at today, and
offer Agenda, Week and Month, switched with 1, 2 and 3. T SHALL go to today,
J and K to the next and previous period, and N SHALL open a new event. Every
shortcut SHALL show its key where the action is, as the rest of the app
does, and be listed in the shortcuts sheet. Times SHALL be shown in the
member's time zone.

#### Scenario: Open the Calendar from the keyboard
- **WHEN** the user presses G then S
- **THEN** the Calendar opens on Agenda at today, marked as the current place in the sidebar

#### Scenario: Next week
- **WHEN** the user is on Week and presses J
- **THEN** the next week shows

### Requirement: Agenda, week and month
Agenda SHALL list days from today onwards, each with its events in time
order, all-day events first; each event SHALL show its kind's icon and
colour, its time, title and place, and a "from email" mark when Scout read
it from an email. Days with nothing SHALL be skipped, and an empty agenda
SHALL say so in one line. Week SHALL be a time grid with an all-day row and a
line at the current time, events side by side when they overlap. Month SHALL
show each day's first events and "+N more". Cancelled events SHALL not show.
Kind colours SHALL be design tokens.

#### Scenario: A day with a flight and a dinner
- **WHEN** Friday 6 November has Scout's flight 07:40–11:55 and Inês's dinner at 20:00
- **THEN** Agenda shows Friday with the flight (plane icon, "from email") and then the dinner

#### Scenario: Nothing ahead
- **WHEN** the member has no events from today onwards
- **THEN** Agenda says "Nothing on your calendar." in one line

### Requirement: The event sheet
Opening an event SHALL show a sheet with its kind as a chip the member can
change, the title, when (with the event's own time zone when it differs from
the member's), the place, the calendar, and, for an event Scout read, "From
<subject> · <sender>", which opens that message in the reader, and the
sentence Scout read the date from. It SHALL offer Edit, Remove (with Undo in
a toast), Accept / Maybe / Decline for an invitation, and, for an event on
the Campbooks calendar, Add to my calendar with a list of the member's
calendars that can be written, the last used first. An event whose change
was not kept SHALL say so ("Changed in Google meanwhile; we kept theirs").
A pending change SHALL show as pending. On phones the sheet SHALL fill the
screen.

#### Scenario: Back to the email
- **WHEN** the user opens Scout's flight event and presses "From Your booking LIS–AMS · Voa Air"
- **THEN** the reader opens that message

#### Scenario: Removing with undo
- **WHEN** the user removes Scout's parcel event
- **THEN** it leaves the calendar and a toast offers Undo for 10 seconds

#### Scenario: A change not kept
- **WHEN** Google refused the user's move because the event changed there
- **THEN** the sheet shows the provider's time and "Changed in Google meanwhile; we kept theirs"

### Requirement: What Scout found, in the reader
Under a message's header the reader SHALL show what was read from it: each
item with its kind's icon, title and time, and "Added to your calendar" with
Undo for an event Scout added, or the calendar it was pushed to. While a read
is queued or running it SHALL say "Scout is reading this…". When the message
is flagged and has not been read, opening it SHALL request the read, once.
When no `complete` role may read mail and the decide pass flagged a dated
thing, it SHALL say "This email has a <kind>" with Add, which opens the new
event form with the kind and the email filled in; for a deadline Add SHALL
open the task form when tasks exist, and not show otherwise. A message with
nothing found SHALL show nothing.

#### Scenario: A flight confirmation
- **WHEN** the user opens the flight email after Scout read it
- **THEN** the strip says "✈ Flight LIS → AMS · Fri 6 Nov, 07:40 · Added to your calendar · Undo"

#### Scenario: Opened before it was read
- **WHEN** the user opens a flagged dentist reminder nobody has read
- **THEN** the strip says "Scout is reading this…" and changes to the appointment when the read arrives, without a reload

#### Scenario: No model for reading
- **WHEN** the server has no `complete` role and the decide pass flagged a booking
- **THEN** the strip says "This email has a reservation" with Add, which opens the event form with Reservation and the email filled in

### Requirement: What is due
Once the Due layer is wired (a follow-up of this change that needs the
`tasks` contracts), the calendar SHALL show the member's open tasks with a
due date as Due rows on their day: "Pay · <title> · <amount>" (the amount
only when the task has one, never guessed) or "Do · <title>"; "Due 17:00"
when due at a time; "Overdue · 2 days" in the danger colour once past. Done
tasks SHALL drop off, with no strike-through. A task with time held on its
due day SHALL show only its held block ("Held · <title>"). In Week, Due
SHALL be its own all-day lane above "All day". Until the Due layer is
wired, no Due row SHALL show.

#### Scenario: A bill due Friday
- **WHEN** Inês has an open pay task "EDP bill" of €84.20 due Friday
- **THEN** Friday shows "Pay · EDP bill · €84.20"

#### Scenario: Overdue
- **WHEN** a do task "Review Marta's proposal" was due two days ago
- **THEN** today shows "Overdue · 2 days" in the danger colour

#### Scenario: Held on its due day
- **WHEN** Inês held 10:00–11:00 on Friday for the task due that Friday
- **THEN** Friday shows "Held · Review Marta's proposal" and no Due row for it

### Requirement: Settings → Calendar
Settings SHALL have a Calendar section with the time zone, working hours per
weekday, the member's calendars with sync on or off and colour, the kinds
Scout adds (each on or off), "Also add to" per kind, and, for each mail
connection without calendar access, "Add calendar access", which starts the
connection's consent for the `calendar` capability.

#### Scenario: Adding calendar access
- **WHEN** the user presses "Add calendar access" next to their Gmail connection
- **THEN** the provider's consent opens for calendar access on that connection, and after it the Google calendars are listed

#### Scenario: Turning off deliveries
- **WHEN** the user turns off Deliveries under "Scout adds these"
- **THEN** later deliveries are no longer added and those already added stay

### Requirement: Calendar on phones and in every language
Every Calendar screen, the event sheet and the reader strip SHALL fit 375px
with no horizontal page overflow (Week scrolls inside its grid only) and
SHALL be translated in English, European Portuguese, Spanish from Spain and
French, with dates and times formatted for the member's language.

#### Scenario: Week on a phone
- **WHEN** Week is open at 375px
- **THEN** the page does not scroll sideways and the grid shows the days that fit, scrolling within itself

#### Scenario: Portuguese
- **WHEN** a Portuguese reader opens Agenda
- **THEN** days read like "sexta-feira, 6 de novembro" and every label is in Portuguese
