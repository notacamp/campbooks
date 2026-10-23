## Why

Campbooks can tell a member that an email needs them, but it cannot hold on to
what they have to do. Today a conversation raises at most one Do or Pay need:
it has no title of its own, no plan, cannot be handed to a teammate and cannot
be written by the member. Daily things that never arrived by email have no
place at all.

The owner asked (2026-10-23) for a simple task system for daily things that
works with the rest of the app: tasks found in email, tasks with due dates, and
tasks that sit on the calendar. It is built together with the calendar
(`schedule`, campbooks-cloud#42) and Scout chat (campbooks-cloud#305), whose
sessions agreed the seams below.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/43.
Prototype (approved 2026-10-23): https://claude.ai/artifact/E2kwAJ19NF1TJVc6gWBoYx

Modules touched:

- **`tasks`**: a new module (crate, `tasks` schema, client module), already in
  `docs/domain-map.md` for M2. It depends on the kernel only, so it needs no
  `architecture.toml` entry.
- **`attention`**: projects open tasks as Do and Pay items, with containers
  that are no longer only inboxes.
- **`assistant`** (only if campbooks-cloud#305 phase 1 is on `next` when this
  lands): Scout chat can read tasks.
- **Client**: a Tasks place, Home's Do and Pay groups, the reader.

No cross-module dependency: `attention` consumes `tasks` events (ADR 0012), and
`tasks` checks a source conversation through the kernel's `authz` catalogue
(ADR 0017).

## What Changes

- **A task.** Something a member has to do, with a title, a verb (Do or Pay),
  an optional due date (a day, or a day and a time), a plan (none, a day, or
  Later until a time), a state (open, done, not needed), one assignee, and an
  optional source: the conversation it came from. Its origin is `own` (the
  member wrote it) in this change. `ask` and `deadline` arrive in the next
  change, from the email read.
- **Operations** `tasks.list`, `tasks.get`, `tasks.create`, `tasks.update`,
  `tasks.plan`, `tasks.done`, `tasks.not_needed`, `tasks.reopen`,
  `tasks.hand_off` and `tasks.take_back`, each change with its undo. Scopes are
  `tasks:read` and `tasks:write`. A history records who changed what: the
  member, Scout, or the calendar.
- **Personal by design.** A task is visible to its assignee, and to the member
  who handed it off, who sees it as "with <name>" and can take it back. There
  are no shared boards, subtasks or priorities.
- **One event**, `tasks.task_changed.v1`, holds a snapshot of ids, enums and
  dates (no title) with a growing revision. `attention` projects it now, and
  the calendar's Due layer will later.
- **Attention.** An open task is a Do or Pay item on its assignee's Home, due
  soonest first. While a task is planned Later it stays off Home. A task made
  from a conversation replaces that conversation's own Do or Pay item, so one
  obligation shows once.
- **Tasks place** (`g t`), as in the prototype:
  - groups: Overdue, Today, Coming up, No date yet, and a folded Later
  - quick add (`n t`) with due chips, and a Do/Pay toggle
  - a detail pane with "When will you do it?" (Pick a day, Later), "Where it
    came from", hand-off by name, and Done/Paid and Not needed
  - every action has a key and an Undo toast
- **From an email:** "Make it a task" (`n t`) in the reader opens an inline form
  with the subject as the title.
- **The `OpenTasks` port.** Open tasks for a conversation and its sender, so
  the single email read (campbooks-cloud#42) can recognise a task that already
  exists instead of creating a second one.

## Non-goals

- Tasks created from email by Scout (asks, deadlines), and moving Do and Pay
  off `conversations`. These come in the next change, from
  `conversations.message_read.v1`, and need no extra AI call: the owner's rule
  is one Jev pass and at most one chat read per email.
- Hold time on the calendar (`TimeHeld`, free slots): this waits for
  `schedule`.
- Due dates on the calendar: `schedule` draws them from `tasks.task_changed`.
- Recurring tasks, subtasks, priorities, projects, shared lists, reminders
  sent by push or email, and typed dates in words ("next Friday").
- Any AI call. This change uses no model.
