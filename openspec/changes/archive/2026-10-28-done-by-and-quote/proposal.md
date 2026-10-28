## Why

"Done?" after a reply (campbooks-cloud#396, PR 1) is asked for every Do task
whose email the member replied to. Some asks a reply cannot finish: sign and
send a form, file something, upload a document, go somewhere. Asking "Done?"
after "I'll sign it tonight" is noise. And the task detail still does not show
the sender's own sentence that asked for the task (campbooks-cloud#397 said it
would arrive with a later change).

Tracking: https://github.com/notacamp/campbooks-cloud/issues/396 and #397.

Modules touched:

- **conversations**: the one read per email says, for each ask, how it gets
  done (`done_by`: `reply` or `elsewhere`), an enum fact with no text. The
  `conversations.message_read` v1 ask gains the optional field.
- **tasks**: `tasks.tasks.done_by` (migration), kept from the event;
  `notice_replies` skips tasks done `elsewhere`; `TaskView` exposes `done_by`
  and `source_ask` (the ask's key: message id and a position, no text).
- **clients/app** (`tasks`, `conversations`): the detail quotes the sender's
  sentence under their name.

## What changes

- The read's strict schema gets `done_by` (required enum) on each ask, with one
  sentence and two contrasting examples in the instruction. No extra model
  call, no new role. A payment is always `elsewhere`: code forces it (a veto on
  the verb, a fact).
- `tasks.add_read_asks` stores `done_by` (NULL for an older read or task).
  `tasks.notice_replies` marks only tasks whose `done_by` is NULL or `reply`.
- The sender's sentence is **not** copied into `tasks.tasks`. It is words from
  the mail, so it is served only by `conversations.get_message_read`, which
  already checks that the caller may read the inbox. The client joins it to the
  task by `source_ask` through the app-level seam (`useQuote`), so a teammate a
  task was handed to who cannot read that inbox gets a 404 and sees no sentence.
  It is in no event and no tracked property.
- The `read-texts` blob is unchanged: the quote already lives in the read.
