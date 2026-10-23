# Tasks from what Scout read in an email

## Why

The first tasks change (#628) gave members their own tasks and let a task
replace its conversation's Do or Pay item on Home. Scout's single read of an
email (#629) now publishes the reader's asks (`conversations.message_read.v1`),
but nothing turns them into tasks, so "Send the signed lease by Friday" or
"Pay invoice 2026-114" still live only as a conversation need with no title,
no plan and no hand-off. The approved prototype (campbooks-cloud#43) makes Do
and Pay on Home tasks.

## What changes

- `tasks` consumes `conversations.message_read.v1`. Each ask in a personal
  inbox becomes an open task for the inbox's owner, origin `ask`, with the
  ask's title, verb, due date and amount, and the conversation as its source.
  An ask that names an open task (`same_as`) adds nothing new; it only gives
  that task a due date when it has none. Repeated deliveries change nothing.
- The read's `read-texts` blob also carries each ask's title and amount, so
  tasks never reads another module's tables and the event keeps no text.
- On Home, a conversation's Do or Pay item stays hidden once its task is
  closed, not only while the task is open, so finishing the task finishes the
  item.
- A task's detail says Scout found it in the email.

No model call is added: tasks reuses the one read per email the owner allowed.

## Non-goals

- Hold time on the calendar (three free slots, `TimeHeld`): the next change,
  once `schedule` exposes a hold.
- Asks in shared inboxes: a task needs one member, so the conversation's need
  stays the item there, and "Make it a task" still works by hand.
- Changing the conversation reader's need chips.
- Tracked events for tasks (campbooks-cloud#340).
