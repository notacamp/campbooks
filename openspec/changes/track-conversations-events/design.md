# Design

How to add a tracked event is in `docs/tracking.md`; the decision is
ADR 0020. This change applies it to the whole `conversations` module.

## Data ownership and events

Every snapshot is read from `conversations`' own rows, in the transaction of
the change, after it: `conversations.conversations`, `.drafts`, `.needs`,
`.rules` and `.rule_runs`. New queries are runtime `sqlx::query_as`. Events
go to `kernel.tracked_events` (the kernel's outbox); nothing else is
written. No domain event is published or consumed that was not before. No
endpoint, scope or AI role changes; Scout's AI work is tracked as what it
stored (a decision, a read), never what it read or wrote.

What an event may hold: ids, enums, booleans, counts and timestamps. Never
a name, address, subject, body, snippet, label name, note, rule condition,
suggestion key, file name or free-text error. A send failure is an error
*kind* (`conversations.send_error`), mapped from the connector's reason;
an unknown reason is `other`.

## Records (snapshots)

| Record | Fields |
| --- | --- |
| `conversations.conversation` (exists) | `id`, `inbox_id`, `stream`, `stream_source`, `status`, `message_count`, `unread_count`, `starred`, `last_message_at` |
| `conversations.draft` | `id`, `inbox_id`, `conversation_id`?, `message_id`? (what it answers), `kind`, `state` (`draft`, `scheduled`, `sending`, `failed`, `sent`), `version`, `to_count`, `cc_count`, `bcc_count`, `attachment_count`, `send_at`?, `failure`? (`send_error`) |
| `conversations.need` | `conversation_id`, `message_id`, `reason`, `question`, `trust`, `basis`, `state` (`open`, `cleared`), `cleared_because`?, `verdict`?, `urgent`, `has_due_date` |
| `conversations.rule` | `id`, `inbox_id`, `state` (as readers see it: `paused` while its author may not act), `paused_reason`?, `origin`, `condition_count`, `actions` (a list of action kinds, never their values), `applied_count` |
| `conversations.rule_run` | `id`, `rule_id`, `kind` (`apply`, `revert`), `state`, `total`, `done`, `changed`, `skipped_suspicious`, `skipped_member` |

`?` marks an optional field. A rule's conditions (senders, domains, subject
templates) and its labels' names are content, so only counts and kinds are
kept.

## Events

All are version 1, `live`, scope `workspace`, added in plan version 2 by
this change (the first two exist since plan version 1 and are unchanged).

### Conversations (`conversations.conversation` snapshot, afterwards)

| Event | Fires when | Other properties |
| --- | --- | --- |
| `conversation_archived` (exists) | `archive` changes a message | |
| `conversation_unarchived` (exists) | `unarchive` changes a message | |
| `conversation_read` | `mark_read` marks a message read | |
| `conversation_marked_unread` | `mark_unread` marks the newest message unread | |
| `conversation_starred` | `star` stars the newest message | |
| `conversation_unstarred` | `unstar` unstars a message | |
| `conversation_trashed` | `trash` moves a message to the trash | |
| `conversation_untrashed` | `untrash` takes a message out of the trash | |
| `conversation_spammed` | `report_spam` moves a message to spam | |
| `conversation_unspammed` | `not_spam` takes a message out of spam | |
| `conversation_moved` | `move_to_stream` changes its stream | `before` (the conversation before the move) |
| `conversation_labelled` | `add_label` puts a label on it that was not there | `label_id`, `label_created` |
| `conversation_unlabelled` | `remove_label` takes a label off | `label_id` |

The flags share `domain::flags::apply`, which already tracks only when a
message changed. A move's `before` is the transition that matters (stream
and who placed it).

### Needs (`conversation`, and `need` afterwards, absent when none is left)

| Event | Fires when |
| --- | --- |
| `need_resolved` | `resolve_need` records a verdict (done, not needed, already paid, still to pay); "still to pay" on a need that was not a question changes nothing and tracks nothing |
| `need_confirmed` | `confirm_need` answers a question yes |
| `need_reopened` | `reopen_need` takes a verdict back |

### Drafts and sends (`conversations.draft` snapshot)

| Event | Fires when | Other properties |
| --- | --- | --- |
| `draft_created` | `create_draft` stores a draft | |
| `draft_updated` | `update_draft` saves a change (a save that changes nothing tracks nothing) | |
| `draft_discarded` | `delete_draft` deletes it (the snapshot as it was) | |
| `draft_scheduled` | `send_draft` schedules it for its undo window | |
| `draft_unscheduled` | `cancel_send` takes a scheduled send back, or `release_send` finds its author may no longer send it | `reason` (`cancelled`, `sender_may_not_send`) |
| `draft_released` | `release_send` hands it to the connector | |
| `draft_sent` | `record_send` hears the provider sent it (the snapshot as it left, state `sent`; the draft is then deleted) | |
| `draft_send_failed` | `record_send` hears it failed, or `check_send` gives up waiting (`unknown_outcome`) | `error` (`send_error`) |

### Rules (`conversations.rule` snapshot, afterwards)

| Event | Fires when | Other properties |
| --- | --- | --- |
| `rule_created` | `create_rule` stores a rule | `acted_without_asking` |
| `rule_confirmed` | `confirm_rule` says yes to a proposal, or binds a kept rule again | |
| `rule_declined` | `decline_rule` turns a proposal down | |
| `rule_updated` | `update_rule` saves it (reshaped, paused, resumed or adopted) | |
| `rule_removed` | `remove_rule` removes it, or a new or confirmed rule replaces it | `cause` (`member`, `replaced`), `put_back` |
| `rule_restored` | `restore_rule` brings it back | |
| `rule_run_finished` | `apply_rule` ends a run (done or cancelled) | `run` (`rule_run`), no `rule` |

A run touches a whole inbox (unbounded), so it is one summary event with
counts when it ends, not one per conversation.

### Suggestions and notes

| Event | Fires when | Properties |
| --- | --- | --- |
| `rule_notice_answered` | `answer_rule_notice` records Not now or Never | `notice_id`, `inbox_id`, `kind`, `answer` |
| `rule_notice_answer_cleared` | `clear_rule_notice_answer` removes an answer | `notice_id`, `inbox_id`, `kind` |
| `rule_suggestion_dismissed` | `dismiss_rule_suggestion` records a member's no | `conversation_id`, `inbox_id`, `times` |
| `person_note_saved` | `set_person_note` creates or changes a note (the same text again tracks nothing) | `note_id`, `created` |
| `person_note_deleted` | `delete_person_note` deletes one | `note_id` |

### Mail, Scout and inboxes

| Event | Fires when | Properties |
| --- | --- | --- |
| `message_batch_applied` | `apply_changes` applies a provider batch that changed anything | `inbox_id`, `backfill`, `messages_added`, `messages_updated`, `messages_removed`, `conversations_changed`, `rules_acted` |
| `message_decided` | the model pass (`decide_message`, `decide_backlog`) stores its answers | `message_id`, `conversation`, `pass` (`arrival`, `backlog`), `answer_count`, `placed`, `moves_only` |
| `message_read_requested` | `request_message_read` queues Scout's read | `message_id`, `conversation_id` |
| `message_read_completed` | `read_message` or `read_structured` stores a read | `message_id`, `conversation_id`, `layer`, `outcome`, `trigger`?, `item_count`, `ask_count`, `summary_ready` |
| `inbox_created` | `create_inbox` creates the inbox a connection's grant names | `inbox_id`, `connection_id`, `member_added` |
| `inbox_closed` | `forget_connection` deletes an inbox's mail or keeps it read-only | `inbox_id`, `mail` (`deleted`, `kept`), `message_count`, `draft_count` |

A batch is bounded by the connector, but holds up to hundreds of messages:
one summary per batch, counts only.

Actor types: an operation's events name `user` and `client`, plus
`assistant` and `automation` where the operation's catalogue entry allows
them; jobs and handlers are `system`, on behalf of whoever caused them.

## Coverage

Operations (`openapi/openapi.json`, tag `conversations`):

| Operation | Events / exemption |
| --- | --- |
| `archive` / `unarchive` | `conversation_archived` / `conversation_unarchived` (exist) |
| `mark_read` / `mark_unread` | `conversation_read` / `conversation_marked_unread` |
| `star` / `unstar` | `conversation_starred` / `conversation_unstarred` |
| `trash` / `untrash` | `conversation_trashed` / `conversation_untrashed` |
| `report_spam` / `not_spam` | `conversation_spammed` / `conversation_unspammed` |
| `move_to_stream` | `conversation_moved` |
| `add_label` / `remove_label` | `conversation_labelled` / `conversation_unlabelled` |
| `resolve_need` / `confirm_need` / `reopen_need` | `need_resolved` / `need_confirmed` / `need_reopened` |
| `create_draft` / `update_draft` / `delete_draft` | `draft_created` / `draft_updated` / `draft_discarded` |
| `send_draft` / `cancel_send` | `draft_scheduled` / `draft_unscheduled` |
| `create_rule` | `rule_created`, `rule_removed` |
| `confirm_rule` | `rule_confirmed`, `rule_removed` |
| `update_rule` | `rule_updated`, `rule_removed` |
| `restore_rule` | `rule_restored`, `rule_removed` |
| `decline_rule` | `rule_declined` |
| `remove_rule` | `rule_removed` |
| `answer_rule_notice` / `clear_rule_notice_answer` | `rule_notice_answered` / `rule_notice_answer_cleared` |
| `dismiss_rule_suggestion` | `rule_suggestion_dismissed` |
| `set_person_note` / `delete_person_note` | `person_note_saved` / `person_note_deleted` |
| `request_message_read` | `message_read_requested` |
| `write_reply`, `stream_reply`, `suggest_replies`, `write_nudge` | exempt: stores nothing; Scout proposes text, and a draft made from it is tracked by `create_draft` |
| `preview_rule` (read) | exempt: a preview changes nothing |

Jobs:

| Job | Events / exemption |
| --- | --- |
| `apply_changes` | `message_batch_applied` |
| `decide_backlog` | `message_decided` |
| `release_send` | `draft_released`, `draft_unscheduled` |
| `check_send` | `draft_send_failed` |
| `apply_rule` | `rule_run_finished` |
| `read_message`, `read_structured` | `message_read_completed` |
| `refresh_need`, `wake_needs`, `backfill_needs` | exempt: derived state, a need worked out again from stored messages and answers (published as `conversations.need_changed`); no action of anyone |
| `notice_rules` | exempt: derived state, the suggestions recounted from recent mail; answering one is tracked |
| `fill_templates` | exempt: maintenance, subject templates filled in for older mail |
| `resign_rules` | exempt: maintenance, rules that predate bindings signed once |
| `retag_bindings` | exempt: maintenance, MACs re-tagged after a key rotation |
| `purge_workspace` | exempt: maintenance, a deleted workspace's rows deleted (the deletion is tracked by `workspaces`) |

Schedules: `dispatch_need_clocks`, `dispatch_needs_backfills`,
`dispatch_rule_resigns`: exempt (maintenance, they only enqueue the jobs
above).

Event handlers:

| Handler | Events / exemption |
| --- | --- |
| `record_send` | `draft_sent`, `draft_send_failed` |
| `decide_message` | `message_decided` |
| `create_inbox` | `inbox_created` |
| `forget_connection` | `inbox_closed` |
| `ingest_messages` | exempt: only enqueues `apply_changes`, which tracks |
| `forget_member` | exempt: consequence of `workspaces.member_left` (tracked there): the departed member's own rows go |
| `pause_suspended_author`, `resume_unsuspended_author` | exempt: projection of `workspaces.member_suspended` / `member_unsuspended`; nothing on the rules changes |
| `schedule_workspace_purge` | exempt: projection of `workspaces.workspace_deleting`, it only schedules the purge |

## Notes

- `rule_removed` fires for each rule a new, confirmed, restored or updated
  rule replaces (`cause: replaced`) in the same transaction, so a
  replacement is two events: the new rule's and the old one's removal.
- `draft_sent` is tracked before the draft row is deleted, so its snapshot
  is read from the module's row; its state is `sent`.
- Patch numbers and plan versions are taken in merge order; a module PR that
  merges after another renumbers its patch and plan version.
