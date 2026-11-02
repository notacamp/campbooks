## The calendar

The Calendar is where the member's days are. Each member has their own calendars. Nobody else sees them, not other members and not the workspace owners.

### Calendars

- Every member has one Campbooks calendar. It exists only in Campbooks. No Google or other provider ever sees it. It holds what Scout found in the member's email.
- A member who gives calendar access to a connected account also gets that account's calendars, such as their main calendar, a family calendar, or holidays. They are read from the provider and changes are written back to it.
- Only the main calendar of an account is shown at first. The member turns the others on in Settings, under Calendar. A calendar that is off shows nothing.
- Calendars shared with the member later, or made later at the provider, show up in Settings, under Calendar, off, within a day. The member turns them on.
- Some provider calendars cannot be written, such as a holidays calendar. Events cannot be added to them.
- Each provider calendar is Mine, Following or Off. Mine is the member's own: its events make them busy, time can be held on it, and invitations on it wait for their answer. Following is shown only: its events do not make the member busy, no time is held on it, and its invitations are not theirs to answer. Off shows nothing. A calendar shared by someone else starts as Following, and the member can change it in Settings, under Calendar. The Campbooks calendar is always Mine.
- A shared inbox has no calendar. Nothing is added from a shared inbox.

### Events and kinds

Every event has one kind: charge, appointment, meeting, travel, stay, show, reservation, delivery, expiry, hold, or none.

- Charge: money will be taken on a date, such as a renewal or a direct debit.
- Appointment: a booked slot. Meeting: a meeting with other people.
- Travel: a flight, a train, a bus or a rental car. Stay: a hotel or a rental home.
- Show: a ticketed event. Reservation: a table or a visit booked.
- Delivery: a parcel arriving. Expiry: something that stops being valid on a date.
- Hold: time kept for a task. None: an event from a provider that nobody gave a kind.

The member can change an event's kind. Events that only have a day are all-day events. Events are shown in the member's own time zone. If the event was written in another zone, that zone is shown too.

### What Scout adds from email

- Scout reads an email once and may find things that happen at a time: a flight, a booking, a delivery, a charge. Each becomes an event on the member's Campbooks calendar, with a link to the email it came from.
- The sender's own calendar data (an invitation, or booking data from an airline) is read without a model. Prose is read by Scout's one read of the email.
- A later email can move or cancel an event that was added. A cancellation email cancels it.
- If the member removes an event Scout added, Scout never adds it again. Undo brings it back.
- The member can turn off kinds in Settings under Calendar, so Scout stops adding them.
- If an invitation is already on the member's provider calendar, Scout links that event to the email and does not add a second one.

### Obligations are not events

Something the member has to do by a date, such as pay by Friday, sign, submit, or renew by a date, is a task. It is shown on the calendar as Due, but it is not an event and it is not in this calendar. Ask about what is due through the tasks, not through events. A task the member planned for a day is shown on the calendar on that day too, as Planned. Ask the tasks (`my_tasks`) for planned and due tasks, not the events.

### Adding to the member's own calendar

- Nothing Scout adds goes to Google or any other provider by itself.
- "Add to my calendar" copies an event to one of the member's provider calendars that can be written. One event remains, on that calendar. The Campbooks calendar no longer shows it.
- The member can set, for a kind, a calendar to also add to. Then every new event of that kind is copied there.
- Removing an event that was added deletes it at the provider too. Undo is offered.

### Changing events

- The member can make, move, rename or delete events on calendars that can be written.
- A series is changed one occurrence at a time. The whole series is not changed from here.
- A change to a provider event is saved first and sent to the provider after. It shows as waiting until the provider takes it. If the event changed at the provider meanwhile, the provider's version is kept and the event says so.
- A delete can be undone for a short time. Within that time the provider does not hear of it.
- Answering an invitation (accept, maybe, decline) is also written to the provider, once its short undo time has passed. Until it is answered, the invitation is also on Home as a Decide need.

### Time zone and working hours

- Each member has a time zone, set from their device the first time and changeable in Settings. Until it is set, UTC is used.
- Working hours are set per weekday. Until the member sets some they are Monday to Friday, 09:00 to 18:00.
- Free time is the part of the working hours that no event overlaps. It counts the Campbooks calendar and the member's own calendars (Mine). Calendars the member follows or has off do not count. It does not count cancelled events, all-day events, or events marked not busy.

### Rules for answering

- Answer only from what the calendar tools return. If an event is not there, say you do not see it.
- Do not guess a date or a time. Times are in the member's time zone.
- Say which calendars a free time answer counted.
- You cannot add, change or delete events from this chat. Tell the member where to do it in the Calendar.
