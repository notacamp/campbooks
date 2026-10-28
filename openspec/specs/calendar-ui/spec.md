# calendar-ui Specification

## Purpose
The Calendar screens of the app: the Calendar place with its agenda, week and
month views, the event sheet, the strip in the reader that says what Scout
found in an email, and Settings → Calendar.

## Requirements

### Requirement: The Calendar place
Calendar SHALL be a place next to Home and Inbox in the sidebar and in the
phone tab bar, opened with G then S (`nav.schedule`, already in the shortcut catalogue). It SHALL open at today on Week, or on Agenda
below the phone breakpoint (48rem) when the address names no view, and a view in the
address SHALL always win. It SHALL offer Agenda, Week and Month, switched with 1, 2 and 3. T SHALL go to today,
J and K to the next and previous period, and N then E (N W, a new workspace,
is global and would hide a lone N) SHALL open a new event. Every
shortcut SHALL show its key where the action is, as the rest of the app
does, and be listed in the shortcuts sheet. Times SHALL be shown in the
member's time zone.

#### Scenario: Open the Calendar from the keyboard
- **WHEN** the user presses G then S
- **THEN** the Calendar opens on Week at today on a wide screen, or on Agenda at today below the phone breakpoint, marked as the current place in the sidebar

#### Scenario: Wide screen default
- **WHEN** the member opens the Calendar at desktop width with no view in the address
- **THEN** Week shows, and the address names no view

#### Scenario: Phone default
- **WHEN** the member opens the Calendar at 375px with no view in the address
- **THEN** Agenda shows, and the address names no view

#### Scenario: A view kept in the address
- **WHEN** the member picks Agenda on a desktop (or Week on a phone) and reloads
- **THEN** the address holds `?view=agenda` (or `?view=week`) and the same view shows again

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

### Requirement: The event sheet
Opening an event SHALL show a sheet with its kind as a chip the member can
change, the title, when (with the event's own time zone when it differs from
the member's), the place, the calendar, and, for an event Scout read, "From
<subject> · <sender>", which opens that message in the reader, and the
sentence Scout read the date from. It SHALL offer Edit and Remove (with Undo in
a toast). On phones the sheet SHALL fill the
screen.

#### Scenario: Back to the email
- **WHEN** the user opens Scout's flight event and presses "From Your booking LIS–AMS · Voa Air"
- **THEN** the reader opens that message

#### Scenario: Removing with undo
- **WHEN** the user removes Scout's parcel event
- **THEN** it leaves the calendar and a toast offers Undo for 10 seconds

### Requirement: What Scout found, in the reader
Under a message's header the reader SHALL show what was read from it: each
item with its kind's icon, title and time, and "Added to your calendar" with
Undo for an event Scout added. While a read
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

### Requirement: Settings → Calendar
Settings SHALL have a Calendar section with the time zone, working hours per
weekday, the member's calendars with colour, and the kinds Scout adds (each
on or off).

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

### Requirement: Provider events in the event sheet
For an event on a provider calendar the sheet SHALL offer Accept / Maybe /
Decline for an invitation. For an event on the Campbooks calendar it SHALL
offer Add to my calendar, shown only when the member has a provider calendar
that can be written, with a list of those calendars, the last used first. An
event whose change was not kept SHALL say so ("Changed in Google meanwhile;
we kept theirs"). A pending change SHALL show as pending. The reader's strip
SHALL name the calendar an event was pushed to.

#### Scenario: A change not kept
- **WHEN** Google refused the user's move because the event changed there
- **THEN** the sheet shows the provider's time and "Changed in Google meanwhile; we kept theirs"

#### Scenario: Adding to a calendar
- **WHEN** the user opens Scout's concert event and has a writable Google calendar
- **THEN** the sheet offers Add to my calendar with that calendar listed

### Requirement: Calendar access and provider calendars in Settings
Settings → Calendar SHALL list the member's calendars in three groups, each
with a one-line description: **Mine** ("Make you busy, hold your tasks, and
send their invitations to Home"), **Following** ("Shown only") and **Off**
("Hidden"); a group with no calendar SHALL not show. Each provider
calendar SHALL have a three-way control (Mine / Following / Off) and its
colour; the Campbooks calendar SHALL show as Mine with no control. A
calendar the provider says the member does not own SHALL say "Shared with
you". Changing a role SHALL move the calendar to its group at once and offer
Undo. The page SHALL also offer "Also add to" per kind, and, for each
connection of theirs that offers `calendar` (the connection's `offers`, as
connectors answers it) and does not hold it, "Add calendar access", which
starts the connection's consent for the `calendar` capability. The page
SHALL NOT decide by provider name which connections are offered it. When
starting is refused it SHALL say why in plain words: a refusal for a sign-in
too old (`step_up_required`) SHALL show a banner with "Sign in again", which
signs in and comes back to Settings → Calendar; a provider this server cannot
add it for, too many tries and any other failure SHALL each have their own
short sentence. It SHALL be in en, pt-PT, es-ES and fr, and fit 375px.

#### Scenario: Adding calendar access
- **WHEN** the user presses "Add calendar access" next to their Gmail connection
- **THEN** the provider's consent opens for calendar access on that connection, and after it the Google calendars are listed

#### Scenario: Following a colleague's calendar
- **WHEN** the user sets "Marta" (shared with them) from Off to Following
- **THEN** "Marta" moves under Following, says "Shared with you", and its events appear on the Calendar

#### Scenario: Making a calendar theirs
- **WHEN** the user sets "Family" from Following to Mine
- **THEN** "Family" moves under Mine, and Undo puts it back under Following

#### Scenario: Zoho when the server offers it
- **WHEN** a Zoho connection without calendar access offers `calendar`
- **THEN** Settings → Calendar shows "Add calendar access" next to it, and shows none while the connection does not offer it

#### Scenario: The sign-in is too old
- **WHEN** the user presses "Add calendar access" and the server answers 403 `step_up_required`
- **THEN** the page shows "Sign in again first" with a "Sign in again" button, not the generic failure, and the button returns to Settings → Calendar after signing in

### Requirement: Held time on the calendar
A held block SHALL show as a Held event ("Held · <title>") on its calendar,
and opening it SHALL offer Open task. A held task SHALL have no Planned row,
and no Due row on the day it is held when that is its due day. The task sheet
of a Do task SHALL offer Hold time (`h`) with the same panel as the task's
detail. Removing the held block SHALL release the task's hold, with Undo
restoring both.

#### Scenario: Held on the planned day
- **WHEN** "Send the signed lease" is held Wednesday 10:00–10:30 and due Thursday
- **THEN** Wednesday shows the Held block at 10:00 and no Planned row, and Thursday shows the Due row

#### Scenario: Removing the block
- **WHEN** Ana removes the held block from the calendar and presses Undo
- **THEN** the block is back and the task shows "Held Wed 10:00–10:30" again

### Requirement: Calendar settings say how adding calendar access came back
Settings → Calendar SHALL read `?connected=` and `?link_error=` when the member
returns from the provider, once, and clear them from the address. On success it
SHALL say calendar access was added, naming the mailbox's address. On
`account_mismatch` it SHALL say the provider answered as another account and
name the mailbox's address to switch to at that provider; when it cannot tell
which mailbox, it SHALL say so without an address. A cancelled return and any
other refusal SHALL each have their own short sentence. It SHALL be in en,
pt-PT, es-ES and fr, and fit 375px.

#### Scenario: Zoho answered as another account
- **WHEN** the member returns to Settings → Calendar with `link_error=account_mismatch` after adding access to their Zoho mailbox
- **THEN** the page says "Zoho answered as another account. Switch to that mailbox's address at Zoho, then try again." and the query is cleared

#### Scenario: Calendar access was added
- **WHEN** the member returns with `connected=<connection>`
- **THEN** the page says calendar access was added for that mailbox's address

### Requirement: Only the member's own calendars take their time
Every place that puts something on one of the member's calendars SHALL offer
only their own (role Mine) calendars that can be written: the calendar for
Hold time, "Add to my calendar", and "Also add to" in Settings. A calendar
set to Following or Off after it was chosen for "Also add to" SHALL show the
choice as unset.

#### Scenario: Holding time
- **WHEN** the user holds time for a task and follows the writable calendar "Marta"
- **THEN** the calendar choice lists their Campbooks calendar and their own writable calendars, and not "Marta"
