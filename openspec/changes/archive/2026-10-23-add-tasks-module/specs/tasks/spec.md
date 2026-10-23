## ADDED Requirements

### Requirement: Tasks scopes
The `tasks` module SHALL declare the scopes `tasks:read` and `tasks:write`,
and every endpoint it adds SHALL require one of them.

#### Scenario: Anonymous caller
- **WHEN** a client calls `GET /api/v1/w/{workspace_id}/tasks` without an access token
- **THEN** the response is 401 as `application/problem+json`

#### Scenario: Token without the write scope
- **WHEN** a client with only `tasks:read` marks a task done
- **THEN** the response is 403 as `application/problem+json`

### Requirement: A task
A task SHALL hold a title (1–200 characters after trimming, with bidi,
zero-width and control characters removed), a verb (`do` or `pay`), an origin
(`own`, `ask` or `deadline`), an optional due date with an optional time, a
plan (`none`, `date` with a day, or `later` with a time), a state (`open`,
`done` or `not_needed`), one assignee, the member who created it, and an
optional source record. Every table of the `tasks` schema SHALL be isolated
per workspace by forced row-level security.

#### Scenario: An own task
- **WHEN** Ana creates "Call the accountant about Q3 VAT", due Friday, with no source
- **THEN** the task is open, `do`, origin `own`, due Friday, plan `none`, assigned to Ana and created by Ana

#### Scenario: A blank title
- **WHEN** a client creates a task whose title is only spaces
- **THEN** the response is 400 and no task is stored

### Requirement: Who sees a task
A task SHALL be readable and changeable by its assignee only, and readable by
the member who handed it off, who may also take it back while it is open. Any
other caller SHALL get 404, whatever the action. Creating a task with a source
SHALL require that the caller may read the source; otherwise the response SHALL
be 404.

#### Scenario: A teammate's task
- **WHEN** João asks for a task assigned to Ana that nobody handed to him
- **THEN** the response is 404

#### Scenario: A source the caller cannot see
- **WHEN** Ana creates a task with a conversation in an inbox she has no role in as its source
- **THEN** the response is 404 and no task is stored

### Requirement: Listing tasks
`tasks.list` SHALL return the caller's tasks, filtered by state, due window,
overdue, undated, source, `mine` or `handed_off`, or by `ids` (1–100; any other
count is 400; unseen ids are left out), with each task's title, verb, due,
plan, state, assignee, source and revision. A handed-off task SHALL be
visible to its giver only while it is open, in a list by `ids` too.

#### Scenario: What is open for me
- **WHEN** Ana lists her open tasks
- **THEN** she gets the tasks assigned to her, overdue first, then by due date and time, then undated, each with its revision

#### Scenario: Handed off
- **WHEN** Ana lists `handed_off`
- **THEN** she gets the open tasks she handed to someone else, with the assignee

### Requirement: The caller's day
Overdue and today SHALL use the caller's UTC offset (the optional `utc_offset`
query parameter) in `tasks.list`, in `tasks.get` and in every write's answer.

#### Scenario: Late evening in Lisbon
- **WHEN** Ana, at UTC+1, reads a task due today at 23:30 her time
- **THEN** it is not overdue in the list, in `tasks.get` or in the answer to a change

### Requirement: Changing a task
`tasks.update` (title, verb, due), `tasks.plan`, `tasks.done`,
`tasks.not_needed`, `tasks.reopen`, `tasks.hand_off` and `tasks.take_back`
SHALL take the revision the caller saw and answer 409 `task_changed` when the
task changed since. Each SHALL declare its undo in `x-campbooks`, append one
history entry, and publish `tasks.task_changed` in the same transaction.
`tasks.hand_off` SHALL accept only a member of the workspace (else 400
`not_a_member`) and record the caller in `handed_by`.

#### Scenario: Done, then undo
- **WHEN** Ana marks "Send the signed lease" done and then presses Undo
- **THEN** the task is open again with its plan, and its history holds `done` then `reopened`

#### Scenario: Two tabs
- **WHEN** Ana plans a task in one tab and marks it done from an older view in another
- **THEN** the second call answers 409 `task_changed` and nothing changes

#### Scenario: Handing it to João
- **WHEN** Ana hands an open task to João
- **THEN** João is the assignee, Ana is `handed_by`, Ana can still read it and take it back, and João's Home shows it

### Requirement: Later
`tasks.plan` with `later` SHALL take a time in the future, and the task SHALL
show under Later until that time and then return to its group by due date with
the plan `none`.

#### Scenario: Next week
- **WHEN** Ana puts "Book the car inspection" off until Monday 09:00
- **THEN** it is under Later until Monday 09:00, and from then on it is listed as undated with no plan

### Requirement: Task history
Each task SHALL keep a history of what changed, when, and by whom (`member`,
`scout` or `calendar`), holding ids, enums and dates only, readable through
`tasks.get`.

#### Scenario: Reading the history
- **WHEN** Ana opens a task she created, planned for Thursday and handed to João
- **THEN** `tasks.get` lists `created`, `planned` and `handed_off` with their times and Ana as the actor

### Requirement: Task changes as events
Every change SHALL publish `tasks.task_changed` v1 holding the workspace, the
task, a growing revision, what changed, the state, verb, origin, due date and
time, plan, the Later time, the assignee, who handed it off, and the source
record. It SHALL hold no title, address or amount.

#### Scenario: Planned for Thursday
- **WHEN** Ana plans a task for Thursday
- **THEN** one `task_changed` event with `change: planned`, `plan: date`, Thursday and the next revision is published, and it contains no title

### Requirement: Who a task left
A hand-off or take-back event SHALL also name the member who had the task
before and the one who had handed it off before, so the change feed removes
it from their lists, and closing a handed-off task SHALL remove it from the
giver's.

#### Scenario: Handed on
- **WHEN** João hands the task Ana gave him to Maria
- **THEN** the event names João and Ana as before, and the task leaves Ana's "with João" list

### Requirement: Open tasks for the email read
The kernel port `OpenTasks` SHALL return, for an inbox, a conversation and a
sender address, at most 10 open tasks, newest first, whose source is that
conversation or whose source sender is that address within that inbox, each
with its record reference, title and due date.

#### Scenario: The same sender in another inbox
- **WHEN** the port is asked about a message from billing@edp.example in the Shared inbox, and Ana's only EDP task came from her personal inbox
- **THEN** the port returns nothing

### Requirement: Tasks workspace deletion
When a workspace is deleted, the `tasks` module SHALL delete its tasks and
history.

#### Scenario: Workspace deleted
- **WHEN** `workspaces.workspace_deleting` is delivered for a workspace
- **THEN** no row of that workspace remains in the `tasks` schema
