## ADDED Requirements

### Requirement: What happened to a conversation
`GET /api/v1/w/{workspace_id}/conversations/{conversation_id}/history`
(`conversations.get_history`, `conversations:read`, not mutating, callable
by the assistant) SHALL answer what the records say happened to a
conversation the caller can read, oldest first, at most 200 events, each
with its time and kind:

- `received` and `sent`: a message, by its id, and who wrote it (name and
  address);
- `scout_read`: one of Scout's answers about a message (question, value,
  source, confidence 0 to 100 when known, whether it acted), read as
  "Suggestions with their reasons" reads them, so model answers without
  their `checked:` stamp are never listed;
- `corrected`: a member changed an answer (question, from, to), by `you`
  when it was the caller, else `member`;
- `rule_applied`, `rule_reverted` and `rule_excepted`: a taught rule, by
  its id and its sentence;
- `need_opened`: the current need (reason, basis, due date); `need_cleared`
  (why, and by `you`, `member`, `rule` or `scout`) and `need_verdict`
  (the verdict, and by whom).

It SHALL answer 404 for a conversation the caller cannot read, as
`conversations.get_conversation` does, and hold no message text.

#### Scenario: A bill
- **WHEN** Marta's rent email arrived, Scout read it as money (91, acted) and needs_reply no (78, acted), and the Pay need opened
- **THEN** the history lists `received`, two `scout_read`, then `need_opened` with reason `pay` and its due date

#### Scenario: Corrected by the caller
- **WHEN** Sam moved the conversation from Updates to Primary
- **THEN** the history lists `corrected` for `stream` from `updates` to `primary` by `you`

#### Scenario: An inbox Sam can't read
- **WHEN** Sam asks for the history of a conversation in an inbox he is not a member of
- **THEN** the answer is 404
