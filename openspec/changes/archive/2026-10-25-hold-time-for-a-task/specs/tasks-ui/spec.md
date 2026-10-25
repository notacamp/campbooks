## ADDED Requirements

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
