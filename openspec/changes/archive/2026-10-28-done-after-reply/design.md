## Context
`conversations` already publishes `need_changed` with `cleared_because: replied` and `message_sent` for every new outbound message. `tasks` reads `message_read` (asks) and the schedule module's events; it knows nothing about replies.

## Decisions
**A fact event, not `need_changed`.** Consuming `need_changed` with `cleared_because = replied` would be less code but misses every reply that comes when no need is open: the need was already cleared by opening the thread, by a rule, or was never raised. The task would then stay asking nothing. The reply itself is the fact, so `conversations` publishes it, and `tasks` decides what it means for its tasks. `message_sent` is not reused: it carries the participants and text blobs and no owner, and its consumers are indexing, not tasks. The new event carries ids and a time only.

**Where it is published.** In `apply_changes`' `announce`, next to `message_sent`, for a new outbound message: the one place an outbound row is inserted (the sent message of a Campbooks send arrives in the connector's change batch like any message, so it is the same path). `announce` only runs for a message stored for the first time, so a repeated batch publishes nothing. Not published for a batch of an inbox's first sync (old mail), nor for a shared inbox (`owner` is the inbox's one reader, `read_message::store::owner`).

**Which tasks.** `tasks.notice_replies` selects open Do tasks with `assignee = owner`, `source = conversations.conversation:<conversation>`, `created_at < sent_at`, and no newer reply already noticed. A reply sent before the task was made (the sync of old sent mail) marks nothing. `replied_at` only moves forward, so a repeated or out-of-order delivery changes nothing. Each marked task writes history (`replied`, actor Scout) and publishes `tasks.task_changed`.

**The question is derived.** `asks_done` is computed from the row (open, Do, `replied_at` set, `reply_answered_at` null or older than `replied_at`), not stored. Not yet stamps `reply_answered_at = greatest(now(), replied_at)`, so a provider clock a little ahead of ours cannot leave the question standing. Its undo clears the stamp. Done closes the task through the existing operation.

**Scout never decides.** No model reads the reply, no content is read at all: the handler reads ids and times (ADR 0018). Neither new operation is offered to the assistant or automation: answering the question is the member's.

**Home.** `attention.items.asks_done` is projected from `task_changed`; `attention.list` treats a task that asks as "for today" so it is not left out under "more in Tasks". Its band follows the existing rules (due, planned, otherwise Whenever). The row is the task's own, so Done and Not yet go through the tasks module's actions with their Undo.

## Data ownership
`tasks` owns `replied_at`, `replied_message` (a conversations message id, by id only, no foreign key) and `reply_answered_at`. `attention` owns `asks_done`. `conversations` owns the event.

## Events
- Published: `conversations.member_replied` v1 (new; `contracts/conversations.member_replied.v1.schema.json`, `openapi/events.json`), `tasks.task_changed` v1 (additive: optional `asks_done`, `replied_at`; `change` gains `replied`, `reply_answered`, `reply_unanswered`; every consumer ignores unknown fields).
- Consumed: `tasks.notice_replies` ← `conversations.member_replied`; `attention.project_task` reads `asks_done`.

## Endpoints and scopes
`POST /api/v1/w/{workspace_id}/tasks/{task_id}/reply_answered` (`tasks.answer_reply`) and `…/reply_unanswered` (`tasks.unanswer_reply`), both `tasks:write`, body `{revision}`, 409 `task_changed` on a stale revision, 409 `no_reply_question` when nothing is asked, 409 `task_closed` on a closed task.

## AI
None. No role is used; there is nothing to fall back from. Every change is the member's and has an Undo.

## Tracked events
`tasks.task_reply_noticed` (handler `tasks.notice_replies`) and `tasks.task_reply_answered` (operation `tasks.answer_reply`); both carry the task snapshot only. Coverage: `tasks.notice_replies` and `tasks.answer_reply` track them; `tasks.unanswer_reply` is exempt (the undo of Not yet). Done is `tasks.task_done`. Patch `tracking/patches/0020-tasks-done-after-reply.yaml`.

## Risks
- A reply that did not do the job: the member answers Not yet, and Scout is quiet until the next reply.
- A teammate's reply from a shared inbox is not noticed; shared inboxes make no tasks anyway.
