## MODIFIED Requirements

### Requirement: Starting a chat
`POST /api/v1/w/{workspace_id}/assistant/chats` (`assistant.start_chat`,
`assistant:write`) SHALL take an optional `about`, a record reference
(`{ type, id }`) of a registered record type. It SHALL check that the member can read that
record (its registry entry's read operations, invoked as the assistant for
the member, answer it), and answer the chat with its messages. When the
member already has a chat about that record, it SHALL answer that chat
(200) instead of making a second one; otherwise it SHALL create one (201).
Without `about`, it SHALL always create a new chat about nothing (201).
It SHALL call no model. A record the member cannot read SHALL answer 404
and create nothing; a type that is not registered SHALL answer 400
`about_unsupported`.

#### Scenario: Continuing
- **WHEN** Sam starts a chat about Marta's conversation, asks two questions, closes the panel and starts a chat about it again
- **THEN** the second answer is 200 with the same chat and its four messages

#### Scenario: A conversation in an inbox Sam can't read
- **WHEN** Sam starts a chat about a conversation of an inbox he is not a member of
- **THEN** the answer is 404 and no chat is created

#### Scenario: A record type nothing registered
- **WHEN** Sam starts a chat about a `conversations.message`
- **THEN** the answer is 400 `about_unsupported` and no chat is created

#### Scenario: Asking from ⌘K twice
- **WHEN** Sam starts two chats without `about`
- **THEN** both answers are 201 with two different chats

### Requirement: What a chat may be about
The chat feature SHALL hold one registry of the record types a chat may be
about. Each entry SHALL name the read operations that describe a record of
its type, invoked as the member through `kernel::operations::invoke` with
`Caller::Assistant` and by operation id only, and a compact projection of
their answers within the bounds of a tool's result. Every operation of the
registry SHALL be in the catalogue with `assistant: true` and
`mutates: false`, checked by a test. The stored type SHALL be checked in
code, so registering a new type needs no migration. The registry SHALL hold
`conversations.conversation` (read through `conversations.get` and
`conversations.get_history`), `conversations.rule` (through
`conversations.get_rule`: its conditions and actions, inbox, origin, state
and how many conversations it acted on) and `conversations.rule_notice`
(through `conversations.get_rule_notice`: its kind, sender, rule and
counts). A chat about nothing SHALL tell the model it is about nothing in
particular.

#### Scenario: A read the member lost
- **WHEN** a registered read answers 403 or 404 on a later question
- **THEN** that question is answered without the record and Scout says it can no longer see it

#### Scenario: A rule of an inbox Sam left
- **WHEN** Sam starts a chat about a rule of an inbox he is no longer a member of
- **THEN** the answer is 404 and no chat is created

## ADDED Requirements

### Requirement: Listing a member's chats
`GET /api/v1/w/{workspace_id}/assistant/chats` (`assistant.list_chats`,
`assistant:read`, not callable by the assistant or automation) SHALL list
the caller's own chats only, newest message first, at most 30, each with
its id, what it is about (type and id, or none), its first question (the
member's own words, cut at 120 characters), how many questions were asked
and the time of its last message. It SHALL read no other module and call no
model. A client token acting for no member SHALL get 403
`chat_needs_member`.

#### Scenario: Only Sam's chats
- **WHEN** Sam and Ana each have two chats and Sam lists his
- **THEN** the answer holds Sam's two chats only, newest first
