# Design

## Owner decisions (2026-10-23)

The owner approved the prototype ("looks good") and set the AI rule that
shapes the next change: **one Jev pass and at most one chat read per email**,
and most mail never reaches the chat model. The prototype's decisions, as
recommended:

1. Do and Pay on Home become tasks (fully in the next change; this change
   makes a task replace its conversation's item).
2. Tasks without a date are allowed. Deadlines found in email always carry
   their date.
3. "Hold time" offers three free slots rather than booking one silently (next
   change, with `schedule`).
4. Holds are written only to the calendar the member picks. Due dates are
   never written to a provider calendar.
5. Tasks are personal: yours, or handed to one teammate. There are no shared
   boards, subtasks or priorities.
6. Quick add takes a title and a due chip, with no dates typed in words.

## Seams agreed with the parallel sessions

- **`schedule` (campbooks-cloud#42):**
  - It renders a read-only "Due" layer from `tasks.task_changed`, and reads
    titles through the `tasks.get` operation as the viewer at render time,
    without copying them.
  - Later: `TimeHeld` / `TimeReleased` (tasks → schedule), and `EventChanged`
    / `EventCancelled` on a hold (schedule → tasks), plus an `Availability`
    port.
- **`conversations` email read (owned by #42's change):**
  - One `complete` read per email at most, gated by Jev's single pass. It
    publishes `conversations.message_read.v1` with `asks[]` (ids, verb, due,
    `has_amount`, `same_as`).
  - Tasks consumes it in the next change and never calls a model itself.
  - The read asks the `OpenTasks` port (built here) for the reader's open
    tasks in that conversation and from that sender, so it can say `same_as`.
- **Scout chat (campbooks-cloud#305):**
  - an anchor registry entry `tasks.task` → `tasks.get`
  - tools `my_tasks` (over `tasks.list`) and `read_task` (over `tasks.get`)
  - a handbook section in `crates/modules/tasks/handbook.md`
    (`pub const HANDBOOK`, ≤ 8 KiB)
  - risk `low` for every write. Phase 2 confirms each change on a card.

## Alternatives considered

- **Grow the conversation need into a task** (title, plan, assignee on
  `conversations.needs`). Rejected for these reasons:
  - a need is one per conversation and belongs to the inbox, while a task
    belongs to a member;
  - own tasks have no conversation;
  - the domain map keeps "need" and "task" apart.
- **Tasks as attention items only.** Rejected: attention holds references with
  no personal data and is rebuilt by replaying events. A task's title and plan
  are the member's own data and need an owner.
- **A `tasks` module (chosen).** It owns the record. Attention projects it like
  any other need. The calendar and Scout read it through operations.
- **One event per change** (`TaskCreated`, `TaskDone`, …, as the domain map
  sketches) **vs one snapshot event.** The snapshot `tasks.task_changed.v1`
  carries a `change` enum and a growing `revision`. Consumers then stay
  order-proof the same way `need_changed` made attention order-proof, and the
  domain map's names survive as `change` values.
- **Later as attention's snooze vs a plan of the task.** A plan, because the
  Tasks page shows it ("Later · Mon 2 Nov"), and the member's Later on Home
  for a task item calls `tasks.plan` instead of `attention.snooze`. Attention
  keeps snoozes for every other item.

## Data (schema `tasks`)

All tables have `workspace_id`, forced row-level security and the
`workspace_isolation` policy (copy `attention__init.sql`, not the scaffold
template). There are no foreign keys across schemas.

- `tasks.tasks`:
  - `id`, `workspace_id`
  - `title` (1–200 chars after trimming, cleaned by `campbooks_kernel::text`)
  - `verb` (`do`|`pay`), `origin` (`own`|`ask`|`deadline`; only `own` is
    written in this change)
  - `due_on` (date), `due_at` (timestamptz when a time is given; then
    `due_on` is its local day)
  - `plan` (`none`|`date`|`later`), `plan_on` (date), `later_until`
    (timestamptz)
  - `state` (`open`|`done`|`not_needed`), `closed_at`
  - `assignee` and `created_by` (members); `handed_by` (member, null unless
    handed off)
  - `source_type`, `source_id`, `source_container` (the inbox, kept for
    `OpenTasks`), `source_sender` (the sender's address, for `OpenTasks`;
    never in an event)
  - `amount_minor`, `currency` (null in this change)
  - `revision`, `created_at`, `updated_at`
  - Index on `(workspace_id, assignee, state, due_on)`, plus partial indexes
    on `source_id` and `source_sender` where `state = 'open'`.
- `tasks.history`:
  - `id`, `workspace_id`, `task_id`, `at`
  - `actor` (`member`|`scout`|`calendar`), `actor_member`
  - `what` (`created`, `renamed`, `due_changed`, `planned`, `done`,
    `not_needed`, `reopened`, `handed_off`, `taken_back`)
  - `detail` (jsonb of ids, enums and dates only)

New SQL uses runtime `sqlx::query_as` with `FromRow`, not the checked macros,
because agents cannot run `cargo sqlx prepare` locally.

## Access (ADR 0017)

Resource type `tasks.task`:
- tenancy: workspace
- owner: the assignee, who may `read` and `write`
- sharing: the member in `handed_by` may `read` it and call `tasks.take_back`
  while it stays open

Everyone else gets `NotFound`. Creating a task with a source requires `read`
on that source through `authz::authorize`; otherwise it is `NotFound`. Hand-off
targets must be members of the workspace; anything else is 400
`not_a_member`. No client-token grants beyond the member's own.

## Operations

| operationId | scope | risk | undo |
| --- | --- | --- | --- |
| `tasks.list` | `tasks:read` | — | — |
| `tasks.get` | `tasks:read` | — | — |
| `tasks.create` | `tasks:write` | low | `tasks.not_needed` |
| `tasks.update` (title, verb, due) | `tasks:write` | low | `tasks.update` with the previous values |
| `tasks.plan` (`none`, `date`, `later`) | `tasks:write` | low | `tasks.plan` with the previous plan |
| `tasks.done` | `tasks:write` | low | `tasks.reopen` |
| `tasks.not_needed` | `tasks:write` | low | `tasks.reopen` |
| `tasks.reopen` | `tasks:write` | low | — |
| `tasks.hand_off` (member) | `tasks:write` | low | `tasks.take_back` |
| `tasks.take_back` | `tasks:write` | low | — |

`tasks.list` filters:
- `state`, `due_from`/`due_to`, `overdue`, `undated`
- `source` (a RecordRef)
- `mine` / `handed_off`
- `ids` (1–100, for Home's batch read)
- `utc_offset` (the caller's day)

Changes made with a stale `revision` answer 409 `task_changed`. Every write
publishes `tasks.task_changed` in its transaction and appends to history.

## Events

`tasks.task_changed.v1` (contract in `contracts/`):
- `workspace`, `task`, `revision`
- `change` (`created`, `edited`, `due_changed`, `planned`, `done`,
  `not_needed`, `reopened`, `handed_off`, `taken_back`)
- `state`, `verb`, `origin`, `due_on`, `due_at`, `plan`, `plan_on`,
  `later_until`
- `assignee`, `handed_by`, `source` (RecordRef or null)

No title, no address, no amount. `changes()` emits the task's RecordRef for
the change feed.

## Attention

`attention.project_task`, consuming `tasks.task_changed`, upserts the item for
record `tasks.task:<id>`:
- reason `do` or `pay`, trust `sure`, `due_on`, no lapse time
- container `tasks.assignee:<member>`, which the `Visibility` port answers
  "only that member"
- cause: the task's source when it has one, else the task

An item is cleared when the task is done, not needed, or handed off (the new
assignee gets their own item). It is kept hidden while `later_until` is in the
future, and comes back on its own once that time has passed. Newer revisions
only; early clears leave tombstones, as `project_conversation_need` does.

Attention changes this requires:
- container types beyond `conversations.inbox`, resolved through the
  `Visibility` port
- the record type no longer hard-coded
- a task item **supersedes** a `do` or `pay` item whose record is the task's
  source conversation: `attention.list` leaves the conversation's item out
  while the task item is open

## Client

- **The `tasks` module** in `clients/app/src/modules/tasks/`:
  - routes `/w/$id/tasks` and `/w/$id/tasks/$task`
  - nav item Tasks (`nav.tasks`, `g t`, already in the catalogue), with the
    open count
- **Screen** as in the prototype:
  - groups Overdue / Today / Coming up / No date yet / Later (folded)
  - quick add with the chips Today, Tomorrow, the next weekday and Pick a
    date, plus Do/Pay
  - list and detail panes at ≥ lg; a single pane with Back below that
- **Keys:** `n t` new task (a global sequence beside `n p`/`n w`; in the reader it makes a task from the email), `j`/`k`, Enter, `e` done (Paid for pay), `s` Later, `x`
  not needed, `z` undo, Esc.
- **Hand-off:** a filter box by name over the workspace's members, never
  typed addresses.
- **Home:** the Do and Pay groups render task items through `tasks.list?ids=`
  with the same row actions (Done/Paid `e`, Later `s`, Not needed `x`).
- **Reader:** "Make it a task" (`n t`) shows an inline form with the subject as
  the title, the due chips and Do/Pay, and calls `tasks.create` with the
  conversation as the source.
- All copy in en, pt-PT, es-ES and fr. It must fit at 375 px (`pnpm
  test:fit`).

## The `OpenTasks` port

`kernel::tasks::OpenTasks::open_tasks(conn, ws, inbox: Uuid, conversation:
Uuid, sender_address)` is the file the calendar change (campbooks-cloud#42)
also defines, taken from there verbatim so the second PR to merge merges
cleanly. It returns at most `MAX_OPEN_TASKS` (10) open tasks, newest first,
whoever holds them, whose source is that conversation or whose
`source_sender` is that address **in that inbox**. Each is an `OpenTask`:
`reference` (the task's id as a hyphenated uuid), `title`, `due_on` and
`due_at`. It is for the email read's prompt only: titles never go into an
event.

## Deviations while building

- **400, not 422.** The kernel has no 422 error, so a blank title, a due time
  without a day and `not_a_member` answer 400 `invalid_title` /
  `due_time_needs_day` / `not_a_member` (problem+json `code`).
- **History kinds** use the event's `change` names: `edited` stands for the
  design's `renamed`.
- **`tasks.update` is a full replacement** of title, verb and due (a missing
  due clears it), so its undo is the same call with the old values.
- **`tasks.list` source filter** is `source_type` + `source_id` query
  parameters, and `ids` is comma separated.
- **`source_sender`** is sent by the caller on `tasks.create` (the kernel
  facts of a source carry its inbox but not its sender); it is stored for
  `OpenTasks` only and never in an event.
- **`previous_assignee`** is an extra optional id on `tasks.task_changed`, so
  the change feed can tell the member a task left, and the member it left on
  Home refreshes.
- **Hand-off access.** `tasks.task` places a handed-off task in the giver's
  `tasks.assignee:<member>` container, which only they see
  (`ContainerViewers`: `read`, and `share` for `tasks.take_back`). This
  needs one `Visibility` port per module, so the kernel gained
  `changes::Visibilities` (dispatch by container type), `authz::facts_of` and
  `OpenTasks`/`AppState::open_tasks`.
- **No contract test** against `contracts/tasks.task_changed.v1.schema.json`
  yet: neither side depends on a JSON Schema validator.
- **No self-undo in the catalogue.** `x-campbooks.undo` may not name the
  operation itself, so `tasks.update` and `tasks.plan` declare none: their undo
  is the same call with the previous values, which the client sends.
  `tasks.reopen` and `tasks.take_back` declare no undo either: they ARE the
  undos (of `done` / `not_needed` and of `hand_off`).
- **`previous_handed_by`** is a second extra optional id on
  `tasks.task_changed`: on a re-hand-off (Ana to João, João to Maria) Ana's
  list gets a delete, and the giver gets an upsert only while the task is open
  (a closed task is a delete, as `tasks.get` 404s it for them).
- **`overdue` follows the caller's day everywhere**: `tasks.get` and every
  write take the same optional `utc_offset` query parameter as `tasks.list`.
- **Error code** `hand_off_yourself` (not `hand_off_to_other`: the catalogue
  reads a trailing `_other` as a plural form).
