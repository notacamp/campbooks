## Where the sender's sentence lives

Options considered for showing the quote on a task:

1. Copy it into `tasks.tasks`. Rejected: it is mail content, and the task is
   visible to a teammate it is handed to who may not read that inbox.
2. Carry it in the `read-texts` blob for the tasks module to store. Rejected
   for the same reason, plus a second copy.
3. Serve it from `conversations` (chosen). The read already stores the quote of
   every ask, vetted by code (an ask whose quote is not in the message is
   dropped), and `conversations.get_message_read` already enforces the access
   rule of the message. A task names its ask by `ask_key`
   (`<message id>:<layer>:ask:<n>`); the message id is the prefix, and the
   conversation is the task's source, so the client needs no new endpoint.
   Exposed on `TaskView` as `source_ask` (ids only; `ask_key` was avoided as a
   name because the catalogue treats `*key*` answers as secrets).

The tasks module never imports conversations: the app plugs `useAskQuote` in as
`useQuote`, like `useSources`.

## done_by

A fact the model reads along with the ask. Code only vetoes: a value outside the
set is no value (NULL, which behaves as before), and a `pay` ask is `elsewhere`
because a reply does not pay a bill. It is an enum, so it may travel in
`conversations.message_read` and be stored in tasks.
