## ADDED Requirements

### Requirement: A task notices the member's reply
`tasks.notice_replies` SHALL handle `conversations.member_replied` by marking
every open Do task assigned to the event's owner, whose source is the event's
conversation and that was made before the reply was sent, with the reply's time
and message. It SHALL only move a mark forward: a reply not newer than the one
noticed changes nothing. It SHALL NOT mark a Pay task, a closed task, a task of
another conversation or of another member. It SHALL read no message: it holds
ids and times only. Each marked task SHALL get a history entry (`replied`, by
Scout), a new revision and a `tasks.task_changed`.

#### Scenario: Ana replies to the email a Do task came from
- **WHEN** Ana replies in the conversation Scout made "Send the signed lease" from
- **THEN** the task holds the reply's time and message, asks "Done?", and its history ends with `replied`

#### Scenario: A bill
- **WHEN** Ana replies in the conversation a Pay task came from
- **THEN** the task is unchanged

#### Scenario: Mail sent before the task existed
- **WHEN** a reply sent before the task was made is delivered
- **THEN** the task is unchanged

#### Scenario: The same delivery twice
- **WHEN** the same `member_replied` is delivered twice
- **THEN** the task changes once

### Requirement: The task asks Done?
A task SHALL ask "Done?" while it is open, a Do, has a reply noticed, and Not yet
has not answered it since: `asks_done` SHALL be true in `TaskView` (with
`replied`, the reply's time and message) and on `tasks.task_changed`. Done SHALL
be the existing `tasks.done`. `POST …/tasks/{task_id}/reply_answered`
(`tasks.answer_reply`, `tasks:write`) SHALL record Not yet: the task stays open,
`asks_done` is false until a newer reply is noticed, and the response is the task
afterwards. `POST …/tasks/{task_id}/reply_unanswered` (`tasks.unanswer_reply`) SHALL
take it back, so the question is asked again. Both SHALL check the revision (409
`task_changed`), refuse a closed task (409 `task_closed`) and a task that asks
nothing (409 `no_reply_question`), and SHALL NOT be offered to the assistant or to
automation. Scout SHALL NOT close a task because of a reply.

#### Scenario: Not yet
- **WHEN** Ana answers Not yet
- **THEN** the task is open, `asks_done` is false, and her answer is in its history

#### Scenario: A newer reply
- **WHEN** Ana answered Not yet and replies in the thread again
- **THEN** the task asks Done? again

#### Scenario: Taken back
- **WHEN** Ana answers Not yet and then Undoes it
- **THEN** the task asks Done? again

#### Scenario: Nothing to answer
- **WHEN** Ana answers Not yet on a task nobody replied to
- **THEN** 409 `no_reply_question`
