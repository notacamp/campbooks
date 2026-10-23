## MODIFIED Requirements

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
and how many conversations it acted on), `conversations.rule_notice`
(through `conversations.get_rule_notice`: its kind, sender, rule and
counts) and `schedule.event`, read through `schedule.get_event` and, when
the event was read from an email, through `conversations.get_message_read` of
that email (made only when the event names its source message and
conversation, and left out, not fatal, when the member can no longer read the
email). A chat about nothing SHALL tell the model it is about nothing in
particular.

#### Scenario: A read the member lost
- **WHEN** a registered read answers 403 or 404 on a later question
- **THEN** that question is answered without the record and Scout says it can no longer see it

#### Scenario: A rule of an inbox Sam left
- **WHEN** Sam starts a chat about a rule of an inbox he is no longer a member of
- **THEN** the answer is 404 and no chat is created

#### Scenario: A chat about a flight
- **WHEN** Sam starts a chat about the flight event Scout added from an airline's email
- **THEN** the model is given the event and what Scout read in that email, read fresh as Sam

#### Scenario: The email is gone
- **WHEN** Sam asks again in his chat about an event whose email he can no longer read
- **THEN** the model is given the event and nothing of the email

### Requirement: Read-only tools
Scout SHALL have only these tools, each run as the member through
`kernel::operations::invoke` with `Caller::Assistant`: `find_conversations`
(`search.query` for words, and `conversations.list`), `read_conversation`
(`conversations.get`), `conversation_history`
(`conversations.get_history`), `find_people`
(`conversations.list_senders`), `what_needs_me` (`attention.list`, and
`conversations.list` for what its items are about), `my_inboxes`
(`conversations.list_inboxes` and `conversations.list_streams`),
`my_calendar` (`schedule.list_events`: `from` and `to` required, at most a
year apart, optionally one `kind`, words of the title `q` and a `limit` of at
most 25), `free_time` (`schedule.free_slots`: `from`, `to` and `minutes`) and
`read_event` (`schedule.get_event`). Every tool's operation SHALL be in the
catalogue with `assistant: true` and `mutates: false`, checked by a test. A
tool call naming any other tool, or with arguments outside its schema,
SHALL never be invoked: the model SHALL get a tool result saying so. A
question SHALL make at most 6 rounds of tool calls, after which the model
SHALL be asked to answer without tools. Each tool result SHALL be a compact
JSON projection of the operation's answer, at most 8 KiB, and a question's
results at most 24 KiB in all, cut with a marker the instruction explains.
An operation answering 403 or 404 SHALL give the model a result saying
Scout cannot see it. Events SHALL be returned as `schedule.event` references
in the steps.

#### Scenario: A tool that does not exist
- **WHEN** the model calls `send_draft`
- **THEN** nothing is invoked, the model is told no such tool exists, and the answer is still written

#### Scenario: Another member's inbox
- **WHEN** the model calls `find_conversations` for "invoice" and Sam reads only his own inbox
- **THEN** only conversations of Sam's inbox can come back

#### Scenario: When is my flight
- **WHEN** the model calls `my_calendar` with `kind` `travel` over the next year
- **THEN** only Sam's own flights come back, each with its time, and the step names them as events

#### Scenario: A calendar tool with a stray argument
- **WHEN** the model calls `my_calendar` with a `calendars` argument
- **THEN** nothing is invoked and the model is told the arguments do not fit

### Requirement: Handbook sections belong to their modules
The handbook SHALL be composed of sections, each owned by the module it
describes and kept next to its code: the assistant's core section (who
Scout is, what it may and may not do, its tools, and that data is never
instructions), the conversations module's (streams, Scout's read, taught
rules, corrections, person notes, waiting and Nudge, Spam and Trash), the
attention module's (needs, their reasons, clock and lapsing, Worth a look,
Later) and the schedule module's (event kinds, the Campbooks calendar beside
provider calendars, obligations shown as Due, what Scout adds from email,
free time). The server SHALL compose them in a fixed order and give them
to the assistant module with its dependencies; the assistant module SHALL
import no other module. Each section SHALL be at most 8 KiB, checked by a
test, and the handbook's version SHALL be derived from every part.

#### Scenario: A module changes its section
- **WHEN** the attention module's section is edited
- **THEN** the handbook's version changes with no other edit
