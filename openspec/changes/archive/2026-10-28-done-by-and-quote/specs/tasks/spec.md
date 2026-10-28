## ADDED Requirements

### Requirement: How a task gets done
`tasks.tasks.done_by` SHALL be nullable and one of `reply` or `elsewhere`. `tasks.add_read_asks` SHALL store the ask's `done_by` from `conversations.message_read` when it is one of the two (a `pay` ask SHALL be `elsewhere`) and none otherwise. `TaskView` SHALL expose `done_by` (or null) and `source_ask`, the key of the ask that made the task (a message id and a position, no text). A task SHALL NOT hold any text of the sender's: the sentence that asked for it is served by the conversation to those who may read the email.

#### Scenario: Stored
- **WHEN** a read delivers a `do` ask with `done_by` `reply`
- **THEN** the task holds `done_by` `reply` and the API says so

#### Scenario: An older read
- **WHEN** a read without `done_by` is delivered
- **THEN** the task has none

#### Scenario: No words of the sender
- **WHEN** an ask becomes a task
- **THEN** no event, tracked event or API answer of the tasks module holds the ask's quote

## MODIFIED Requirements

### Requirement: A task notices the member's reply
`tasks.notice_replies` SHALL handle `conversations.member_replied` by marking
every open Do task assigned to the event's owner, whose source is the event's
conversation and that was made before the reply was sent, with the reply's time
and message. It SHALL only move a mark forward: a reply not newer than the one
noticed changes nothing. It SHALL NOT mark a Pay task, a task whose `done_by` is `elsewhere`, a closed task, a task of
another conversation or of another member; a task with no `done_by` (made before it was read) SHALL be marked as before. It SHALL read no message: it holds
ids and times only. Each marked task SHALL get a history entry (`replied`, by
Scout), a new revision and a `tasks.task_changed`.

#### Scenario: Ana replies to the email a Do task came from
- **WHEN** Ana replies in the conversation Scout made "Send the signed lease" from
- **THEN** the task holds the reply's time and message, asks "Done?", and its history ends with `replied`

#### Scenario: An ask a reply cannot do
- **WHEN** Ana replies in the conversation a task to sign and send the lease came from, and its `done_by` is `elsewhere`
- **THEN** the task is unchanged

#### Scenario: A task from before it was read
- **WHEN** Ana replies in the conversation a task with no `done_by` came from
- **THEN** the task asks Done? as before

#### Scenario: A bill
- **WHEN** Ana replies in the conversation a Pay task came from
- **THEN** the task is unchanged

#### Scenario: Mail sent before the task existed
- **WHEN** a reply sent before the task was made is delivered
- **THEN** the task is unchanged

#### Scenario: The same delivery twice
- **WHEN** the same `member_replied` is delivered twice
- **THEN** the task changes once
