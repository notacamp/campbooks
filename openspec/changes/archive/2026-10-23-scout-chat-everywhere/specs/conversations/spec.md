## ADDED Requirements

### Requirement: Reading one of Scout's suggestions
`GET /api/v1/w/{workspace_id}/rule-notices/{notice_id}`
(`conversations.get_rule_notice`, `conversations:read`, a query callable
by the assistant) SHALL answer one open suggestion as
`conversations.list_rule_notices` lists it, for an inbox the caller may
write in and not hidden from them, else 404. It SHALL not start a notice
pass.

#### Scenario: A suggestion Sam hid
- **WHEN** Sam answered Never to a suggestion and asks for it by id
- **THEN** the answer is 404
