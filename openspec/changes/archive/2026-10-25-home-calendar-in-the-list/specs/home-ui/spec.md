## MODIFIED Requirements

### Requirement: What needs you
When the user can see at least one inbox, Home SHALL show their open
attention items (`attention.list`) in groups by priority band, in the order
Now, Today, This week, Whenever, leaving out empty groups, each item in the
list's order. Each group SHALL be headed by its name and its count of items,
Now in the danger tone; events from the calendar (see *Next up* and *Today's
calendar in the list*) are never counted, and a Today that holds only events
SHALL be headed by its name alone. The hero SHALL say how many things need
the user, with the Visor `alert` when a Review item is on the list. The
selected item SHALL be shown by the selected background and a full focus
ring, never by a coloured stripe on one edge. Each item SHALL show, before
its name, an eyebrow line with its kind and why it is in its band (see *The
reason first*), then who it is from, its sentence (from a template chosen by
reason and by the rule that decided it, in the user's language), the
sender's sentence quoted on one line, and, at the row's end, how long it has
waited. The age SHALL count from when the item's cause arrived (`since`), in
the user's language: minutes or hours the same day, "Yesterday" and days ago
in the week after, the date from a week on (with the year when it is another
year's), with the full date and time on hover; every row SHALL show it,
dated or not. Rows SHALL come from the owning module's batch read
(`conversations.list` with `ids`), not from `attention`. The keys (↑/↓ and
the rest) SHALL follow the order on screen, events included.

#### Scenario: A mixed morning
- **WHEN** the user has a phishing warning, a bill overdue by a day, a reply due tomorrow, a dated ask due Friday and a reply with no date
- **THEN** Home says "5 things need you", Now holds the warning and the bill, Today the reply due tomorrow, This week the ask, and nothing shows a percentage

#### Scenario: Due soonest first
- **WHEN** This week holds a question due Wednesday and an older one with no date
- **THEN** the one due Wednesday is first, and its eyebrow says "Due Wed"

#### Scenario: How long it has waited
- **WHEN** Marta's question arrived at 10:00 today, Pedro's on Monday and the bill on 2 October, and it is Friday 15:00
- **THEN** Marta's row says "5 hours ago", Pedro's "4 days ago" and the bill's "2 Oct", each with its full date on hover, and the bill's due date is in its eyebrow

#### Scenario: A nudge after the bills
- **WHEN** the user has a bill overdue by a day, a nudge and a reply from a service with no date
- **THEN** Home says "3 things need you" and lists the bill in Now, the nudge in Today, then the reply in Whenever

#### Scenario: Down the bands with the keys
- **WHEN** the user presses ↓ on the last item of Now
- **THEN** the first item of Today is selected

#### Scenario: Events are not counted
- **WHEN** the user has two bills overdue, nothing else on the list, and three events later today
- **THEN** Home says "2 things need you", and Today is headed "Today" with no count over the three events

## ADDED Requirements

### Requirement: Next up
Above Now, Home SHALL show a group "Next up" holding one event: the user's
earliest event on their calendars (`schedule.list_events`, the calendars
they show) that is not all-day, not held time (`hold`), not cancelled and
not declined, and that starts within the next 120 minutes or started less
than 10 minutes ago, on the calendar's clock (the user's zone, as the
Calendar screen). There SHALL be no group when there is no such event, and
Home SHALL work it out again each minute, so the event leaves 10 minutes
after it started. The row SHALL show, in place of an avatar, a square in the
kind's colour with the kind's icon (as the calendar draws kinds); an eyebrow
with the kind's name and how soon ("Meeting · In 20 min", "In 1 h 10 min",
"Now", "Started 5 min ago"); the title; the place, the host of a web address
or else the calendar's name, under it; and its time at the end ("11:20 –
11:50"). Selected, it SHALL offer Join (1) only when its place is an
`https://` address, which opens it; Open email when it was read from an
email, which opens it in the reader; and Open in Calendar (Enter), which
opens Calendar on that day with the event open.

#### Scenario: A call in 20 minutes
- **WHEN** it is 11:00 and the user has a meeting at 11:20 with the place `https://meet.example.com/kxr-4471-tmp`
- **THEN** Next up shows "Meeting · In 20 min", its title, "meet.example.com" and "11:20 – 11:50", and Join opens the address

#### Scenario: Too far ahead
- **WHEN** the user's next event starts in 121 minutes
- **THEN** Home shows no Next up

#### Scenario: Just started
- **WHEN** the user's meeting started 5 minutes ago
- **THEN** it is still Next up, saying "Started 5 min ago", and it leaves Next up 10 minutes after its start

#### Scenario: Not these
- **WHEN** the next things on the calendar are an all-day holiday, time held for a task and a meeting the user declined
- **THEN** none of them is Next up

### Requirement: Today's calendar in the list
After the Today band's items, Home SHALL list the rest of today's events
that start from now on, held time (`hold`) and all-day events covering
today, leaving out the Next up event and cancelled and declined events,
all-day first, then by start time, each as an event row (see *Next up*)
whose eyebrow is the kind's name and its time ("Reservation · 12:30",
"Held · 15:00", "Delivery · All day"). Today SHALL show for them even when
no item is in it. Event rows SHALL take part in ↑/↓ like items, and Enter on
one SHALL open it in Calendar on its day. They SHALL never count in the
hero's "N things need you", the Home badge or a group's count. When today
has a timed event that is not held time, the hero's line SHALL say the
day's facts instead of its general sentence: "Next: <title> at <time>." and
"N more on your calendar today." when there are more, in the user's
language, no AI involved. Without events Home SHALL look as it did.

#### Scenario: The rest of the day
- **WHEN** it is 11:00, Marta's reply is due tomorrow, and the user has a call at 11:20, lunch at 12:30, time held at 15:00 and a key handover at 18:00
- **THEN** Today lists Marta's reply, then "Reservation · 12:30", "Held · 15:00" and "Appointment · 18:00", Next up holds the call, and the hero says "Next: <call> at 11:20. 3 more on your calendar today."

#### Scenario: Onto an event with the keys
- **WHEN** the user presses ↓ on Today's last item, then Enter
- **THEN** the first of today's events is selected, then Calendar opens on today with that event open

#### Scenario: Nothing on the calendar
- **WHEN** the user has no event this week
- **THEN** Home shows no Next up, no event rows and no Coming up, and the hero's line is as before

### Requirement: Coming up
First in Home's aside (before Waiting on others and Done today; under the
list on a phone), Home SHALL show "Coming up", "The next 7 days", with up to
6 lines from tomorrow to a week after it, soonest first: the user's events
of any kind and the tasks due on those days (the calendar's Due layer, not
the days tasks are planned for), all-day and untimed first on a day. Each
line SHALL show its short day ("Wed 14"), a dot in its kind's colour (a
ring, named "Only you see it…" for screen readers, when only the user sees
the event), the title, and at its end its time or "Due" for a task, or, for
an event only the user sees that one of their provider calendars can take
(not held time), "Add to my calendar", which adds it with the existing push
to the calendar they added to last (else their primary writable one) and
shows the toast "Added to <calendar>" with Undo. Under the lines, "Open
Calendar" SHALL open Calendar, showing its keys (`G` `S`). Coming up SHALL
not show when the week holds no event.

#### Scenario: A delivery only the user sees
- **WHEN** a furniture delivery read from mail is on Wednesday on the user's Campbooks calendar only, and their Work calendar can take it
- **THEN** its line reads "Wed 14", a ring, the title and "Add to my calendar"; pressing it adds it to Work, the toast says "Added to Work" with Undo, and the line then shows its time

#### Scenario: The week's order
- **WHEN** the week holds a charge on Thursday (all day), a task due Friday, a renewal on Saturday and a train on Sunday at 08:15
- **THEN** Coming up lists them in that order, the task's line saying "Due"

#### Scenario: At most six
- **WHEN** the week holds eight events
- **THEN** Coming up shows the six soonest
