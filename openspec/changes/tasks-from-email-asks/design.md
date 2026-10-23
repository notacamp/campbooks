# Design

## One owner per ask

`schedule` reads only `items` from `message_read` and never `asks`
(confirmed with the calendar session on 5cf937ca), so every `do` and `pay` ask
is owned by `tasks`. Dated tasks reach the calendar later through the Due
layer, which reads `tasks.task_changed`.

## Where the ask's words come from

The event carries no text by contract. The options were:

1. **Add an `asks` section to the `read-texts` v1 blob (chosen).** The blob is
   already the message-owned home for a read's words, is deleted with the
   message, and is how `schedule` gets item titles. The section is optional,
   so v1 readers that ignore it are unaffected. The blob is now written when
   any item or ask has text.
2. Read `conversations.message_reads` from `tasks`: crosses schemas, which the
   architecture forbids.
3. Call `conversations.get_message_read` from the consumer: an operation is
   made for a caller, and a background job has none.

The task copies the title (and amount) because a task's title is the member's
own data from then on; the quote stays with the message.

## Idempotency and merging

- A new nullable `ask_key` column with a unique index per workspace makes a
  repeated delivery a no-op, as `schedule.events.item_key` does.
- `same_as` names an open task of the reader (the read was given the list by
  the `OpenTasks` port). The consumer checks that the task exists, is open,
  is the owner's and came from the same conversation or sender inbox; if so it
  stores nothing new except a due date the task did not have. A `same_as` that
  fails the check is treated as no `same_as`.
- The structured layer (an invoice's `pay` ask) can run before the model
  layer of the same message. The model layer sees that task through
  `OpenTasks` and answers `same_as`; as a second guard, a `pay` ask from the
  model layer of a message whose structured layer already made a `pay` task
  for the owner is that task.
- Scout never overrides the member: it only fills an empty due date, and
  never reopens a closed task.

## Who gets the task

The event's `owner` is the inbox's personal owner. With no owner (a shared
inbox) the consumer does nothing, as `schedule` does.

## Home

The list already hides a conversation's `do`/`pay` item while the caller has
an open task item from that conversation. Once that task is done or not
needed the conversation item came back, because the task item was cleared.
The rule becomes: the caller has a task item from that conversation that is
open, or was cleared at or after the conversation item's `since`. A newer
ask (a later message) opens a newer conversation item and, through the read,
a newer task.
