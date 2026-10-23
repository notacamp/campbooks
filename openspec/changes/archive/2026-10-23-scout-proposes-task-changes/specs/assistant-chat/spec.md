## ADDED Requirements

### Requirement: Proposing task changes
Scout SHALL have a fourth proposing tool, `propose_task_change`, taking
`action`, `task_id` and, only when the action takes one, `day`
(`YYYY-MM-DD`), `until` (RFC 3339) or `member_id`. `action` SHALL be
`done`, `not_needed`, `plan_day`, `later`, `due` or `hand_off`. It SHALL NOT
create or rename a task. A call SHALL only be checked and stored.

#### Scenario: Scout proposes marking a bill paid
- **WHEN** Ana says she paid the electricity bill and the model calls `propose_task_change` with `done` for that Pay task
- **THEN** Scout's message holds a pending proposal for `tasks.done` with the undo `tasks.reopen`, and the task is still open

#### Scenario: Renaming is not offered
- **WHEN** the model calls the tool with `rename` or with a `title`
- **THEN** nothing is stored and the model is told the arguments do not fit the tool

### Requirement: Task actions and their undos
Each task action SHALL be one row of the action table: `done`
`tasks.done`, `not_needed` `tasks.not_needed` (undo `tasks.reopen`),
`hand_off` `tasks.hand_off` (undo `tasks.take_back`), and the restoring rows
`plan_day` and `later` (`tasks.plan`) and `due` (`tasks.update`, the
record's title and verb with the new due day). Each SHALL be `assistant`,
`mutates` and `risk: low` in the catalogue, checked by a test.

#### Scenario: A plan put back
- **WHEN** the model proposes `plan_day` 2026-10-23 for a task planned on 2026-10-20
- **THEN** the undo is `tasks.plan` with plan `date` on 2026-10-20

#### Scenario: A task with no due day
- **WHEN** the model proposes `due` for a task with no due day
- **THEN** the undo is `tasks.update` with the task's title and verb and no due day

### Requirement: Task requests are conditional on the revision
A task proposal's request SHALL carry the task's `revision` read when it
was proposed, and its undo SHALL name `revision` in `from_response`, so the
client copies the revision the change left and nothing else of the answer.
`tasks.update` SHALL be the one proposed request whose method is `PATCH`.

#### Scenario: Changed in between
- **WHEN** Ana confirms a card after the task changed elsewhere
- **THEN** the request answers 409 `task_changed` and the card says it could not be done

### Requirement: Checking a task proposal
The server SHALL read the task and the workspace's members as the member
and SHALL refuse with a tool result naming why: `not_visible`; `read_only`
for a task the member handed off; `already_so`; `task_closed` for a closed
task; `until_out_of_range` for an `until` not later than now or a year away;
`day_out_of_range`; `not_a_teammate`.

#### Scenario: Someone else's task
- **WHEN** the model proposes `done` for a task that is João's and was never handed to Ana
- **THEN** the model is told `not_visible` and no card is stored

#### Scenario: A task Ana handed off
- **WHEN** the model proposes `done` for a task Ana handed to Rui
- **THEN** the model is told `read_only`

#### Scenario: Handing it to herself
- **WHEN** the model proposes `hand_off` to Ana's own user id
- **THEN** the model is told `not_a_teammate`

### Requirement: Days and teammates
A `day` SHALL be today or later in the member's time zone and less than a
year away. A `hand_off` `member_id` SHALL be a member of the workspace
other than the member, given as their user id or their full name as the
members list gives it.

#### Scenario: Yesterday
- **WHEN** the model proposes `plan_day` for yesterday in Ana's time zone
- **THEN** the model is told `day_out_of_range`

### Requirement: The shared cap
`propose_task_change` SHALL count toward the same three proposals per
answer as the other proposing tools, and an identical proposal in one answer
SHALL be stored once, whichever tools the others came from.

#### Scenario: Four cards asked
- **WHEN** the model proposes `done` twice, an archive, a rule, then `plan_day`
- **THEN** three cards are stored, the repeated `done` once, and `plan_day` is told `too_many`

### Requirement: What a task proposal holds
A task proposal SHALL hold the task as a record reference, the task's
title, verb and due day read from its record, its action, its detail (`day`,
`until`, or for a hand-off the teammate's id and the name the members list
gives), and its request and undo. Nothing on the card SHALL be the model's
words, and a task's title SHALL never be logged.

#### Scenario: Facts, not words
- **WHEN** the model proposes handing a task to "Rui" and writes "the urgent bank task" in its text
- **THEN** the card names the task's own title and "Rui Matos" from the members list
