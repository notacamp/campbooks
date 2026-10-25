## ADDED Requirements

### Requirement: The assistant, attention, search and system modules' changes are tracked
Every change the `assistant`, `attention` and `system` modules make, and the
kernel's job queue changes an administrator makes through `system`, SHALL be
tracked as an event of the tracking plan, in the transaction of the change,
with a snapshot read from the owning module's own rows: chats with Scout
started, continued (a question stored), answered and deleted; Scout's
proposals confirmed, declined and undone; a member's profile updated; an
item put off until later and brought back; a workspace's first refused AI
call of the day; a dead job retried or discarded, the dead jobs of a kind
retried, and a schedule's stuck run discarded. A change that changes
nothing SHALL track nothing. Events SHALL carry ids, record references,
enums, booleans, counts, dates and times only: never a question, an answer,
a prompt, a title or anything a model wrote, a search query, a screen
route, a job's payload or error. A search query, a read and the search
index's upkeep SHALL track nothing. Every mutating operation, job, schedule
and handler of the four modules SHALL have a Coverage row.

#### Scenario: A member asks Scout in a chat
- **WHEN** a member starts a chat about a conversation and asks a question
- **THEN** `assistant.chat_started`, `assistant.chat_continued` and `assistant.answer_finished` are written with actor `user`, the chat's id, what it is about as a record reference, its question count, and the answer's finish and tools; none holds the question or the answer
- **AND** starting the chat about the same conversation again tracks nothing

#### Scenario: A member declines a proposal
- **WHEN** a member says Not now to one of Scout's cards
- **THEN** one `assistant.proposal_declined` event is written with the proposal's kind, action and state `declined`
- **AND** settling it again is refused and tracks nothing

#### Scenario: A member puts an item off
- **WHEN** a member snoozes an item until Monday, then until Tuesday
- **THEN** two `attention.item_snoozed` events are written, the second with `before` Monday
- **AND** snoozing it until Tuesday again tracks nothing

#### Scenario: An administrator retries a dead job
- **WHEN** an administrator retries a dead job
- **THEN** one `kernel.job_retried` event is written with actor `admin`, operation `system.admin_retry_job`, and the job's kind, queue and state `pending`
- **AND** retrying it again tracks nothing

#### Scenario: A workspace reaches its daily AI limit
- **WHEN** a workspace's background AI calls reach the cap less the reserve
- **THEN** one `system.ai_limit_reached` event is written in that workspace with the day, the cap, the reserve and `interactive` false
- **AND** later refusals the same day track nothing

#### Scenario: A member searches
- **WHEN** a member searches their mail
- **THEN** nothing is tracked
