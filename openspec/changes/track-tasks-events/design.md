# Design

How to add a tracked event is in `docs/tracking.md`; the decision is
ADR 0020. This change applies it to the tasks module
(`crates/modules/tasks/`).

## Data ownership and events

The module owns the schema `tasks`. Every snapshot is read from its own
row (`tasks.tasks`) in the transaction of the change, by
`domain/tracked.rs`. Every event belongs to the task's workspace. Events go
to `kernel.tracked_events`; nothing else is written. No domain event,
endpoint, scope or AI role changes; the task's history and
`tasks.task_changed` stay as they are. New queries are runtime
`sqlx::query_as` (no `.sqlx` file changes).

What an event may hold: the task's id, the user ids of its assignee, its
creator and the member who handed it off (pseudonymous), the record it came
from by reference (`conversations.conversation` and its id), enums (verb,
origin, state, plan), dates and times (due, planned day, end of Later,
closed, created, updated), its revision, and whether it has an amount.
Never its title, the sender's address it keeps for matching, the amount or
its currency, the ask's key (it embeds a message id and the read's layer),
the inbox it lives in, or any quote from the email.

- **Amount**: a `pay` task read from an email may carry the amount the
  email asked for. The amount is what someone owes: the snapshot says
  `has_amount`, never the sum or the currency.
- **Source**: the record a task came from is a reference (record type and
  id); a task the member wrote with none has none. The inbox
  (`source_container`) and the sender (`source_sender`) are left out.

## Events

All workspace events. Property names never repeat an envelope field: the
task is `task`, the other member `previous_assignee` /
`previous_handed_by`.

| Event | Fires when | Actor | Snapshot and properties |
| --- | --- | --- | --- |
| `tasks.task_created` | `tasks.create` makes a task, or `tasks.add_read_asks` makes one of an ask Scout read in an email to the inbox's owner | `user`, `assistant`, `automation`, `system` | `task` |
| `tasks.task_edited` | `tasks.update` changes the title or the verb; the same values again track nothing | `user`, `assistant`, `automation` | `task` (after), `fields` (list of `tasks.task_field`), `before_verb`? (when the verb changed) |
| `tasks.task_due_changed` | `tasks.update` changes the due day or time, or `tasks.add_read_asks` gives an open task the due date a later ask names when it had none | `user`, `assistant`, `automation`, `system` | `task` (after), `before` (`tasks.task_due`) |
| `tasks.task_planned` | `tasks.plan` stores another plan than the task had (a day, Later until a time, or none); the same plan again tracks nothing | `user`, `assistant`, `automation` | `task` (after), `before` (`tasks.task_plan`) |
| `tasks.task_done` | `tasks.done` closes an open task (paid, for a bill) | `user`, `assistant`, `automation` | `task` (after) |
| `tasks.task_not_needed` | `tasks.not_needed` closes an open task as not needed | `user`, `assistant`, `automation` | `task` (after) |
| `tasks.task_reopened` | `tasks.reopen` opens a closed task again | `user`, `assistant`, `automation` | `task` (after), `before` (`tasks.task_state`: done or not needed) |
| `tasks.task_reassigned` | `tasks.hand_off` gives an open task to a teammate, or `tasks.take_back` gives a handed-off task back to the member who handed it | `user`, `assistant`, `automation` | `task` (after: on a hand-off `assignee` the teammate, `handed_by` the giver), `reassignment` (`tasks.reassignment`: handed_off, taken_back), `previous_assignee` (who had it), `previous_handed_by`? (who had handed it off before this change, if anyone) |

A client token acts for no member: it can make no task and see none, so
`client` is no actor type of these events. Scout's own asks
(`tasks.add_read_asks`) run as the handler, so their actor is `system` on
behalf of whoever caused the read.

Records:

- `tasks.task`: `id`, `verb`, `origin`, `state`, `due_on`?, `due_at`?,
  `plan`, `plan_on`?, `later_until`?, `assignee` (pseudonymous),
  `created_by` (pseudonymous), `handed_by`? (pseudonymous), `source`?
  (record reference), `has_amount`, `revision`, `closed_at`?,
  `created_at`, `updated_at`.
- `tasks.task_due`: `due_on`?, `due_at`?.
- `tasks.task_plan`: `plan`, `plan_on`?, `later_until`?.

The plan is the stored one: a Later whose time has passed still reads as
`later` with its `later_until` (the API shows it as no plan).

Enums: `tasks.task_verb` (do, pay), `tasks.task_origin` (own, ask,
deadline), `tasks.task_state` (open, done, not_needed), `tasks.plan_kind`
(none, date, later), `tasks.task_field` (title, verb),
`tasks.reassignment` (handed_off, taken_back).

### Handed off and taken back: one event

The plan's rule wants an action that is a past participle (`handed_off`
and `taken_back` end in `off` and `back`), so a hand-off and a take-back
are both `tasks.task_reassigned`, told apart by `reassignment`
(`handed_off`, `taken_back`, the module's own words). Both move the task
to another member, and both carry who had it (`previous_assignee`) and,
for a re-hand-off, who had handed it to them (`previous_handed_by`).

### Edited and due changed

One `PATCH` that renames a task and moves its due date tracks both
`task_edited` (`fields: [title]`) and `task_due_changed`: the due date is
the transition that matters (it drives Home's Do and Pay), so it has its
own event with the date before, as schedule's moved events do.

### A plan set again

`tasks.plan` stores what it is sent, and the module appends a history
entry and bumps the revision even when the plan is the one the task had.
Nothing changed, so nothing is tracked; the module's behaviour is
unchanged.

### Where each is tracked

Every change of a task ends in `domain::finish` (history, then
`tasks.task_changed`, in the change's transaction). Each caller now passes
the row as it was before the change (`Outcome::before`; none for a new
task), and `finish` calls `tracked::track`, which compares it with the row
afterwards and tracks the events above. `add_read_asks` locks the task
before giving it a due date, so `before` is what the update changed.

## Coverage

Operations (`openapi/openapi.json`, tag `tasks`; the reads `tasks.list`
and `tasks.get` need no row):

| Operation | Events |
| --- | --- |
| `tasks.create` | `task_created` |
| `tasks.update` | `task_edited`, `task_due_changed` |
| `tasks.plan` | `task_planned` |
| `tasks.done` | `task_done` |
| `tasks.not_needed` | `task_not_needed` |
| `tasks.reopen` | `task_reopened` |
| `tasks.hand_off` | `task_reassigned` |
| `tasks.take_back` | `task_reassigned` |

Event handlers:

| Handler | Events / exemption |
| --- | --- |
| `tasks.add_read_asks` | `task_created`, `task_due_changed` |
| `tasks.schedule_workspace_purge` | exempt: projection of workspaces.workspace_deleted: it only schedules the purge |

Jobs:

| Job | Events / exemption |
| --- | --- |
| `tasks.purge_workspace` | exempt: maintenance: a deleted workspace's tasks and history deleted; the deletion is tracked by workspaces |

The module has no schedule (cron). `attention.project_task`, the
projection of `tasks.task_changed` into Home, already has its row
(exempt) and is unchanged.
