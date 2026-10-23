## Why

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/43

Scout reads a member's tasks in a chat (`my_tasks`, `read_task`) and, since
`scout-proposes-actions`, proposes changes to conversations on cards the
member confirms. A member who says "I paid the electricity bill, and I will
send Marta the lease on Friday" still has to leave the chat and find each
task. The tasks module left the seam open: its operations are low-risk,
opted in for the assistant, with an undo or the values to restore.

## What changes

- A fourth proposing tool, `propose_task_change`, kept apart from
  `propose_change` (whose arguments are conversation-shaped). It takes
  `action`, `task_id` and, when the action takes one, `day`, `until` or
  `member_id`. Actions: `done` (a bill reads "Mark paid"), `not_needed`,
  `plan_day`, `later`, `due`, `hand_off`.
- Each action is one row of the existing action table over a tasks
  operation. `done` and `not_needed` undo with `tasks.reopen`, `hand_off`
  with `tasks.take_back`; `plan_day`, `later` (both `tasks.plan`) and `due`
  (`tasks.update`) are restoring rows holding the task's values read when
  proposed. Every request carries the `revision` read then, and every undo
  copies the `revision` the change left (`from_response`), so a task that
  changed in between answers 409 instead of being overwritten.
- The server reads the task and the workspace's members as the member
  before storing, shares the three-cards-per-answer cap and the dedupe with
  every other proposing tool, and builds the card's facts (title, verb, due
  day, the teammate's name) from the records.
- The client runs the new operations from its own list (a task's edit is
  the one PATCH), words the cards in four languages, and refreshes the
  Tasks and Home queries after Confirm and Undo.
- No new model call: only a tool definition in the chat call that exists.

## Non-goals

- Creating a task or renaming one: a title would be the model's words on a
  record. `tasks.create` stays off the proposing list.
- Changes without a yes, taking a task back (the giver reads and takes
  back by hand), reopening from a card, or editing more than the due day.
- The "Ask Scout" entry on a task and chats about a task as the opener:
  they wait for phase 3 (#637) and are a separate change.

## Capabilities

### Modified capabilities

- `assistant-chat`: the task proposing tool, its checks, what a task
  proposal holds.

## Modules

`assistant` (server and client). It reads `tasks.get` and
`workspaces.list_members` by operation id through the catalogue, like its
other reads; no new module dependency and no new operation or event, so
nothing for the tracked-events coverage.
