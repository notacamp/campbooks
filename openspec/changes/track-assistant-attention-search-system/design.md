# Design

How to add a tracked event is in `docs/tracking.md`; the decision is
ADR 0020. This change applies it to four small modules at once:
`assistant`, `attention`, `search` and `system`.

## Data ownership and events

Every snapshot is read from the owning module's own rows, in the
transaction of the change:

- `assistant`: `assistant.chats`, `.chat_messages` (Scout's answer: how it
  finished, the tools of its steps, how many proposals), the proposals
  stored on a Scout message, `assistant.profiles`.
- `attention`: `attention.items` with the caller's `attention.snoozes`.
- `system`: `system.ai_usage_days`, in the meter's own transaction.
- `kernel`: `kernel.jobs`, in the kernel's transaction (below).

New queries are runtime `sqlx::query_as`; no query the modules already have
changes its text. Events go to `kernel.tracked_events`; nothing else is
written. No domain event is published or consumed that was not before, and
no endpoint, scope, migration or AI role changes.

What an event may hold: ids, record references, enums, booleans, counts,
dates and timestamps. A chat is content: never a question, an answer, a
prompt, a chat's first question, a title, or anything the model wrote (a
rule's sentence, a reply's intent, a label, an event's title, place or
times, a teammate's name). Which tool Scout used and which change it
proposed are enums of the code (`assistant.chat_tool`,
`assistant.proposal_action`), never the model's words. Which profile
settings changed is an enum (`proactivity`, `asked`); proactivity itself is
an enum of two choices and is in the snapshot. A job is its kind and queue
(codes from the code, matching `^…$` patterns), never its payload or its
last error.

## Decisions

- **Chat "continued" is a question asked.** `assistant.start_chat` answers
  200 "continued" for a chat about the same record the member has already:
  nothing is written, so it is a read and tracks nothing. What changes a
  chat is a question: `chat_continued` is tracked when the question is
  stored (the chat's `questions` count grows), and `answer_finished` when
  Scout's answer is stored, answered, cut short or failed. Two events,
  because they are two transactions: a streamed answer the member hangs up
  on is never finished, and its question still counts.
- **Proposals.** One event per settlement the member makes:
  `proposal_confirmed` (pending to `done`, or to `failed` when their own
  request was refused; the snapshot's state says which), `proposal_declined`
  (pending to `declined`) and `proposal_undone` (`done` to `undone`). A
  settlement the card cannot take is a 409 and tracks nothing. The change
  itself is the member's own request and is tracked by its module
  (`conversations.conversation_archived`, …).
- **Profile.** `profile_updated` names the settings that changed. A first
  save with the defaults, or the values the profile already has, changes
  nothing the member sees (only `revision` grows) and tracks nothing.
- **Later.** `item_snoozed` carries `before`, the Later it replaced (none
  for a new one), and `item_unsnoozed` the Later it ended. Snoozing until
  the same time again, or unsnoozing what is not snoozed, tracks nothing.
- **The admin job actions are the kernel's events.** `kernel.jobs` is
  written only through the kernel (`jobs::admin`), in its own
  transaction, and snapshots come from the owning module's rows, so the
  events are `kernel.*`, tracked inside `jobs::admin::{retry, retry_kind,
  discard, discard_schedule}`, which now take the tracker (the
  `system.admin_*` operations pass `state.tracker`). They are instance-wide
  (the kernel's transaction works in no workspace); a job's workspace is a
  field of its snapshot. The actor is the administrator (`admin`, from the
  `Admin` extractor). An outcome of `unchanged` or `superseded` changed
  nothing and tracks nothing.
- **A bulk retry is one summary event.** `system.admin_retry_jobs` retries
  every dead job of a kind (at most 1,000 per call): its input names a
  filter, not ids, so it tracks one `kernel.jobs_retried` with the kind,
  the queue filter, how many were requeued and whether more remain. None
  requeued tracks nothing.
- **The AI daily limit.** The meter (`system::AiUsageMeter`) is no
  operation, job, schedule or handler: it runs inside whatever makes an AI
  call. Counting calls is bookkeeping and tracks nothing. The first refused
  call of a workspace's UTC day is tracked (`system.ai_limit_reached`):
  from then until midnight UTC the workspace's new mail is sorted by rules
  only, which its members notice, and it is the number the daily cap
  (#505) is tuned by. Its actor is whoever made the refused call (the
  member asking Scout, the mail job). The meter is composed with the
  server's tracker: `ai_gateway` and `Composition::from_env` take it, and
  the server builds its tracker (with its role) before composing.
- **Streamed answers keep their context.** `assistant.stream_chat` writes
  the answer after the handler returned, while the body streams, outside
  the request's task-local tracking context. The work now runs in the
  context captured in the handler (`tracking::scope`), so its events, and
  the AI meter's, are the member's, with the operation and correlation of
  their request.
- **Search tracks nothing.** A query is a read and its text never leaves
  the module; the index is a projection of the sources' events, built,
  refreshed and purged in the background. No operation lets anyone start a
  reindex; the operator's `search reindex` CLI command rebuilds a
  projection and is outside the Coverage kinds.
- **Telemetry is not tracked again.** `system.report_client_telemetry` and
  `system.report_screen_view` are telemetry themselves (forwarded, scrubbed,
  to the operator's collector and usage analytics, never stored): recording
  them as tracked events too would count every screen twice.

## Records (snapshots)

| Record | Fields |
| --- | --- |
| `assistant.chat` | `id`, `about`? (record reference), `questions`, `created_at`, `last_message_at` |
| `assistant.answer` | `id`, `chat_id`, `finish` (`done`, `cut_short`, `failed`), `tools` (list of `assistant.chat_tool`), `proposals`, `failure`? (`assistant.answer_failure`) |
| `assistant.proposal` | `id`, `chat_id`, `kind` (`change`, `rule`, `reply`), `state` (`pending`, `done`, `declined`, `failed`, `undone`), `action`? (`assistant.proposal_action`), `record`? (record reference) |
| `assistant.profile` | `user_id`, `proactivity` (`ask_first`, `act_then_tell`), `asked`, `asked_at`?, `revision` |
| `attention.item` | `id`, `record` (record reference), `reason` (`review`, `reply`, `decide`, `do`, `pay`, `nudge`, `worth_a_look`, `waiting`), `trust` (`sure`, `check`), `due_on`?, `urgent`, `state` (`open`, `cleared`), `since`, `snoozed_until`? |
| `kernel.job` | `id`, `kind` (code), `queue` (code), `workspace_id`?, `state` (`pending`, `running`, `failed`), `attempts`, `max_attempts`, `created_at` |

`?` marks an optional field. Record references (`about`, `record`) are
`{type, id}` of the conversation, task, rule or calendar event, classified
`id`; `user_id` and `workspace_id` are `pseudonymous`. `assistant.chat_tool`
is the tools of `ChatTool` (`find_conversations`, `read_conversation`,
`conversation_history`, `find_people`, `what_needs_me`, `my_inboxes`,
`my_tasks`, `read_task`, `my_calendar`, `free_time`, `read_event`);
`assistant.proposal_action` the 31 changes of `ProposalAction`;
`assistant.answer_failure` the answer loop's kinds of failure (`over_cap`,
`rounds`, `timed_out`, `provider`) and the gateway's (`timeout`,
`transport`, `status`, `invalid_response`, `config`,
`external_content_refused`, `role_not_configured`, `answers_not_trusted`,
`unattributed`, `metering`), never the provider's words. An
attention item's `snoozed_until` is the acting member's own Later.

## Events

All are version 1, `live`, added in plan version 7 by this change.

### assistant (workspace; `user` or `client`)

| Event | Fires when | Properties |
| --- | --- | --- |
| `chat_started` | `assistant.start_chat` makes a new chat; continuing the one about the same record tracks nothing | `chat` |
| `chat_continued` | `assistant.ask_chat` or `assistant.stream_chat` stores the member's question (never its text) | `chat` (with the new `questions`) |
| `answer_finished` | Scout's answer to that question is stored: answered, cut short, or failed (with why) | `answer` |
| `chat_deleted` | `assistant.delete_chat` deletes the chat and its messages | `chat` (as it stood) |
| `proposal_confirmed` | `assistant.settle_proposal` settles a pending card `done`, or `failed` when the member's own request was refused | `proposal` |
| `proposal_declined` | `assistant.settle_proposal` settles a pending card `declined` (Not now) | `proposal` |
| `proposal_undone` | `assistant.settle_proposal` settles a done card `undone` | `proposal` |
| `profile_updated` | `assistant.update_profile` changes proactivity or whether the questions were answered; the same values track nothing | `profile`, `fields` (list of `proactivity`, `asked`) |

### attention (workspace; `user`, `client`, `assistant`, `automation`)

| Event | Fires when | Properties |
| --- | --- | --- |
| `item_snoozed` | `attention.snooze` puts an item off until a new time for the caller | `item`, `before`? (the Later it replaced) |
| `item_unsnoozed` | `attention.unsnooze` ends the caller's Later of an item | `item`, `before` (until when it was put off) |

### system (workspace; whoever made the refused call)

| Event | Fires when | Properties |
| --- | --- | --- |
| `ai_limit_reached` | the meter refuses a workspace's first AI call of a UTC day: background work at the cap less the reserve, or a person's call at the whole cap | `day`, `cap`, `reserve`, `interactive`, `ai_role` (`decide`, `embed`, `complete`, `vision`) |

### kernel (instance; `admin`)

| Event | Fires when | Properties |
| --- | --- | --- |
| `job_retried` | `system.admin_retry_job` puts a dead job back on its queue; a job not dead, or superseded, tracks nothing | `job` |
| `jobs_retried` | `system.admin_retry_jobs` requeues at least one dead job of a kind | `kind`, `queue`?, `requeued`, `more` |
| `job_discarded` | `system.admin_discard_job` deletes a dead job; one not dead, or gone, tracks nothing | `job` (as it stood) |
| `schedule_run_discarded` | `system.admin_discard_schedule_run` deletes a schedule's pending or stuck row; nothing to discard tracks nothing | `job` (the row, as it stood) |

## Coverage

### assistant

| Kind | Name | Events, or exemption |
| --- | --- | --- |
| operation | `assistant.start_chat` | `chat_started` |
| operation | `assistant.ask_chat` | `chat_continued`, `answer_finished` |
| operation | `assistant.stream_chat` | `chat_continued`, `answer_finished` |
| operation | `assistant.delete_chat` | `chat_deleted` |
| operation | `assistant.settle_proposal` | `proposal_confirmed`, `proposal_declined`, `proposal_undone` |
| operation | `assistant.update_profile` | `profile_updated` |
| operation | `assistant.compile_instruction` | exempt: stores nothing: Scout proposes rules from a sentence, and a rule saved from them is tracked by `conversations.create_rule` |
| handler | `assistant.schedule_workspace_purge` | exempt: projection of `workspaces.workspace_deleted`: it only schedules the purge |
| handler | `assistant.forget_member` | exempt: consequence of `workspaces.member_left`, tracked there: the departed member's profile and chats go |
| job | `assistant.purge_workspace` | exempt: maintenance: a deleted workspace's profiles and chats deleted; the deletion is tracked by workspaces |
| schedule | `assistant.dispatch_expire_chats` | exempt: maintenance, no action of anyone: only enqueues the jobs that do the work |
| job | `assistant.expire_chats` | exempt: maintenance, no action of anyone: chats 30 days past their last message deleted with their messages (retention, not anyone's request) |

### attention

| Kind | Name | Events, or exemption |
| --- | --- | --- |
| operation | `attention.snooze` | `item_snoozed` |
| operation | `attention.unsnooze` | `item_unsnoozed` |
| handler | `attention.project_conversation_need` | exempt: projection of `conversations.need_changed`: one item per conversation's need (a new message from the sender ends its Laters) |
| handler | `attention.project_task` | exempt: projection of `tasks.task_changed`: one item per task |
| handler | `attention.schedule_workspace_purge` | exempt: projection of `workspaces.workspace_deleted`: it only schedules the purge |
| handler | `attention.forget_member` | exempt: consequence of `workspaces.member_left`, tracked there: the departed member's Laters go |
| handler | `attention.forget_inbox` | exempt: consequence of `conversations.inbox_closed`, tracked there: the removed inbox's items and their Laters go |
| job | `attention.purge_workspace` | exempt: maintenance: a deleted workspace's items deleted; the deletion is tracked by workspaces |

### search

| Kind | Name | Events, or exemption |
| --- | --- | --- |
| handler | `search.on_message_received` | exempt: projection of `conversations.message_received`: the conversation's entries in the index refreshed |
| handler | `search.on_message_sent` | exempt: projection of `conversations.message_sent`: the conversation's entries in the index refreshed |
| handler | `search.on_conversation_changed` | exempt: projection of `conversations.conversation_changed`: the conversation's entries in the index refreshed |
| handler | `search.on_inbox_removed` | exempt: projection of `conversations.inbox_removed` (tracked as `conversations.inbox_closed`): the inbox's entries go |
| handler | `search.schedule_workspace_purge` | exempt: projection of `workspaces.workspace_deleted`: it only stops indexing and schedules the purge |
| job | `search.purge_workspace` | exempt: maintenance: a deleted workspace's index deleted; the deletion is tracked by workspaces |
| job | `search.backfill` | exempt: projection: a workspace's index built from its sources a page at a time; no action of anyone |

`search.query` is a read: no row.

### system

| Kind | Name | Events, or exemption |
| --- | --- | --- |
| operation | `system.report_client_telemetry` | exempt: telemetry itself: the app's error and timing reports, forwarded scrubbed to the operator's collector and never stored; tracking them would record them twice |
| operation | `system.report_screen_view` | exempt: telemetry itself: a screen the app shows, relayed to the operator's usage analytics and never stored; tracking it would record it twice |
| operation | `system.admin_retry_job` | `kernel.job_retried` |
| operation | `system.admin_retry_jobs` | `kernel.jobs_retried` |
| operation | `system.admin_discard_job` | `kernel.job_discarded` |
| operation | `system.admin_discard_schedule_run` | `kernel.schedule_run_discarded` |

The module has no job, schedule or handler. Its reads (`health`, `version`,
`admin_overview`, `admin_ai_usage`, `admin_jobs`, `admin_failed_jobs`) need
no row.

## Notes

- `answer_finished` reads the stored answer back: its `finish`, the tools of
  its steps in order, how many proposals it holds. A failure's kind is the
  word the answer loop logs (`Failure::kind`, `answer::error_kind`); a word
  the plan does not know leaves `failure` out rather than guessing.
- Deleting a chat reads it before the delete, in the same transaction:
  `chat_deleted` carries the chat as it stood.
- The meter's refusal is tracked in the transaction that counts it
  (`refused = 1` on the day's row); a later refusal the same day tracks
  nothing. With no cap the meter never refuses.
- Patch numbers and plan versions are taken in merge order; this one builds
  on plan version 6 (`0006-connectors`, campbooks-cloud#329).
