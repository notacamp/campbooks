## Why

A task in the detail says where it came from, but not why it is here: who made
it and when, whether Scout made it, who handed it over. What happened to it
(planned, held, moved by the calendar, replied to, done) is recorded by the
server in `tasks.get`'s `history` and never shown. A member who finds a task
Scout made, or one a teammate handed over, cannot tell how it got there.

Tracking: https://github.com/notacamp/campbooks-cloud/issues/397. Approved
prototype board "Task detail: why it's here + history".

Modules touched:

- **Client only** (`clients/app`, module `tasks`): the task detail reads
  `history` from `tasks.get` and shows "Why it's here" and "History". No
  server, migration, contract, tracked event or prompt change.

## What changes

- "Where it came from" becomes "Why it's here": a sentence by origin (Scout
  made it, you made it, a teammate made it, who handed it to you and when),
  then the email card as before.
- A "History" list below it, oldest first, one plain sentence per entry with
  the time and a dot by who did it (member, Scout, calendar). More than eight
  entries fold to the first and the latest six with "Show all".
- The sender's own words are not shown yet; they arrive with a later change.
