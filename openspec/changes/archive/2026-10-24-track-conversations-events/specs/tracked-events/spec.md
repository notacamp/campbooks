## ADDED Requirements

### Requirement: The conversations module's changes are tracked
Every change the `conversations` module makes on someone's behalf SHALL be
tracked as an event of the tracking plan, in the transaction of the change,
with a snapshot of the records it touched read from the module's own rows
after the change: a conversation's flags, stream and labels; its need's
verdicts; drafts and their sends; taught rules and their runs; rule
suggestions and person notes; applied provider batches; the model pass and
Scout's reads; inboxes created and closed. A change that changes nothing
SHALL track nothing. Events SHALL carry ids, enums, booleans, counts and
times only: never a name, address, subject, body, snippet, label name, note,
rule condition or free-text error. Every operation, job, schedule and event
handler of the module SHALL have a Coverage row.

#### Scenario: A member stars a conversation
- **WHEN** Sam stars a conversation that has no starred message
- **THEN** one `conversations.conversation_starred` event is written with actor `user` (Sam), operation `conversations.star` and the conversation afterwards (starred)

#### Scenario: Starring it again
- **WHEN** Sam stars the same conversation again
- **THEN** nothing more is tracked

#### Scenario: A send fails at the provider
- **WHEN** the connector reports that a released send failed with `needs_reauth`
- **THEN** one `conversations.draft_send_failed` event is written with actor `system`, error `needs_reauth` and the draft's ids, kind, state and recipient counts, and no recipient address or text

#### Scenario: A rule run ends
- **WHEN** applying a taught rule to an inbox's mail finishes
- **THEN** one `conversations.rule_run_finished` event is written with the run's counts, not one per conversation
