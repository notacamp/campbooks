# Ask "Done?" after you reply to the email a task came from

## Why
Scout makes a Do task from an ask in an email. The member then replies to that email, the conversation's need clears (`replied`), and the task stays open: the tasks module never hears about replies, and nothing asks "is it done?". Home keeps nagging about a job the member may already have finished.

Modules: `conversations`, `tasks` and `attention` (server); `tasks`, `attention` and `conversations` (client). Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/396. This is the first of two changes: Scout deciding by itself that the reply did the job (reading the reply) is the second and is not here.

## What changes
- **A fact for the reply.** `conversations` publishes `conversations.member_replied` v1 once for each outbound message stored for the first time in a conversation of a personal inbox, whether the member sent it from Campbooks or from the provider's own client. Ids and a time only. No event for a shared inbox, nor for the mail an inbox's first sync reads.
- **Tasks notice it.** A new handler `tasks.notice_replies` marks every open Do task of the owner that came from that conversation and was made before the reply with the reply's time and message (only moving forward). Pay tasks are never marked.
- **The question.** A task "asks Done?" while it is an open Do with a reply noticed that "Not yet" has not answered since. `TaskView` gains `replied` and `asks_done`; `tasks.task_changed` gains optional `asks_done` and `replied_at`. Done is the existing `tasks.done`. Not yet is new: `tasks.answer_reply`, with `tasks.unanswer_reply` as its undo. The question comes back only after a newer reply.
- **Home, Tasks, the thread.** Home shows the task first in its row ("You replied · 2 hours ago", "You answered Marta's email. Is this task done?") with Done (1), Not yet (2), the thread (O) and Ask Scout (`.`), and says why Scout asks when it opens. A task asking Done? stays on Home whatever its plan. The Tasks list marks the row, the task's detail asks above where it came from, and the thread's "where things stand" note asks too.
- **Scout's handbook** says a task from an email asks "Done?" after the member replies, and that Scout does not close it.

## Non-goals
- Scout closing the task itself, or reading the reply to decide (the second change).
- Pay tasks: a reply does not pay a bill.
- Tasks of a shared inbox or of a teammate: only the inbox owner's own tasks are marked.
- A new band or notification for the question.

## Impact
- Server: migrations `tasks.tasks` (`replied_at`, `replied_message`, `reply_answered_at`, wider history check) and `attention.items` (`asks_done`). One new event (`conversations.member_replied`), one new handler (`tasks.notice_replies`), two operations (`tasks.answer_reply`, `tasks.unanswer_reply`), three new `change` values on `tasks.task_changed` (`replied`, `reply_answered`, `reply_unanswered`). No new scope. A new shortcut `item.open` (O) in the client registry.
- Client: the Home row, the Tasks row chip and detail question, the thread note, in four languages.
