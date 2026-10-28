## ADDED Requirements

### Requirement: Tasks track the Done? question
`tasks.notice_replies` SHALL track `tasks.task_reply_noticed` and `tasks.answer_reply`
SHALL track `tasks.task_reply_answered`, each with the task snapshot only. `tasks.unanswer_reply`
SHALL be exempt, as the undo of the answer.

#### Scenario: Not yet
- **WHEN** Ana answers Not yet
- **THEN** one `tasks.task_reply_answered` is tracked with the task and no title
