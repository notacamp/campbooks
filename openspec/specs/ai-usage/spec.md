# ai-usage Specification

## Purpose
How much AI each workspace uses, and the daily limit that bounds it: every
model call is attributed and counted per workspace, role, feature and UTC
day, background work stops short of the limit so people keep a reserve, and
a workspace past it gets no model call until the next day, so a flood of
mail cannot run up an open bill (campbooks-cloud#152).

## Requirements

### Requirement: Every model call is attributed and metered
Every model call (`chat`, `chat_stream`, `embed`, `decide`) through a gateway
composed with a meter SHALL be attributed to a workspace and a feature, or
to the instance, before anything is sent. The server SHALL compose its
gateway with the `system` module's meter. A call with no attribution SHALL
be refused with nothing sent. Instance calls (the `decide` answers check)
SHALL NOT count against any workspace. One logical call SHALL count once,
whatever the retries after a connection failure or a 5xx answer. A streamed
call SHALL be admitted, or refused over the cap, before its request is
sent, within the caller's usage scope.

#### Scenario: A call path that forgot to say whose it is
- **WHEN** code calls a metered gateway outside any usage scope
- **THEN** the call fails as unattributed and no request reaches the provider

#### Scenario: The answers check
- **WHEN** the `decide` answers check runs on a metered gateway
- **THEN** its calls are made under the instance scope and no workspace's usage changes

#### Scenario: The composed server
- **WHEN** the server composes its modules with an AI role configured
- **THEN** the gateway it hands to the modules is metered

#### Scenario: A streamed call over the cap
- **WHEN** a workspace past its daily limit asks for a streamed reply
- **THEN** the call is refused with `OverDailyCap` and no request reaches the provider

### Requirement: Usage ledger
The `system` module SHALL record, per workspace and UTC day, the model calls
admitted and refused, and per workspace, day, role and feature, the calls,
the failed calls and the prompt and completion tokens the provider reported.
The ledger SHALL hold counts and ids only, never content, and SHALL be
isolated per workspace by row-level security, readable across workspaces
only in an administrator's transaction.

#### Scenario: Calls by role and feature
- **WHEN** a workspace's mail is read by the `decide` pass three times in a day
- **THEN** its ledger shows three `decide` calls for `conversations.triage` that day

#### Scenario: Tokens and failures
- **WHEN** a provider reports tokens for a call, or a call fails
- **THEN** the tokens are added to that call's role and feature for the day it was admitted, or its failure is counted

#### Scenario: Two workspaces
- **WHEN** two workspaces use AI on the same day
- **THEN** each has its own counters and neither can read the other's

### Requirement: Daily cap per workspace
A workspace SHALL be admitted at most `CAMPBOOKS_AI__DAILY_CALLS_PER_WORKSPACE`
model calls per UTC day (5,000 when unset; `0` means no cap), across every
role and feature. Background calls SHALL stop a reserve short of the cap (a
tenth of it, rounded up, leaving background work at least one call); calls a
person is waiting on may use the whole cap. Admission SHALL be atomic, so concurrent workers and
processes never admit more than the cap. A call over the cap SHALL be
refused with nothing sent. The first refusal of a workspace's day SHALL log
one line saying that the workspace reached its daily AI limit and that its
new mail is sorted by rules only until 00:00 UTC, with the workspace id and
the cap and no content. The count SHALL start again at 00:00 UTC.

#### Scenario: Under the cap
- **WHEN** a workspace with a cap of 3 makes its first two background calls of the day
- **THEN** both are sent

#### Scenario: At the background limit
- **WHEN** it makes a third background call that day
- **THEN** the call is refused, nothing is sent, the refusal is counted, and the limit is logged once

#### Scenario: People keep a reserve
- **WHEN** a person's call (the assistant) comes after background work used its share of a cap of 3
- **THEN** it is sent, and the next one that day is refused

#### Scenario: Concurrent calls
- **WHEN** many interactive calls of one workspace are admitted at the same time with a cap of 5
- **THEN** exactly 5 are admitted

#### Scenario: The next day
- **WHEN** the UTC day changes
- **THEN** the workspace's calls are admitted again

#### Scenario: No cap
- **WHEN** the cap is configured as 0
- **THEN** every call is admitted and still counted

### Requirement: Over the cap, mail gets no model pass
When the `decide` pass (a new message, or a stored one from the backlog) is
refused over the cap, it SHALL store nothing and finish without error,
exactly as when no model is configured: the message keeps what the facts and
taught rules decided when it was stored. A skipped message SHALL NOT be
re-queued, so the next day's cap is spent on that day's mail.

#### Scenario: The message after the cap
- **WHEN** a message arrives after its workspace's cap is reached
- **THEN** it is stored as the facts and taught rules left it, has no model answers, and no job fails

#### Scenario: No burst at midnight
- **WHEN** the day changes after messages were skipped over the cap
- **THEN** no model call is made for the skipped messages

### Requirement: Metering failures never uncap
When the ledger cannot be written before a call, the call SHALL NOT be sent.
The `decide` pass SHALL retry the message later, so storing and rule-sorting
are never blocked and the model's answers are not lost. When only recording
the tokens after a call fails, the call SHALL still count.

#### Scenario: The ledger is unavailable
- **WHEN** admitting a call fails with a database error
- **THEN** nothing is sent and the `decide` pass asks again later

### Requirement: The assistant past the daily limit
Compiling a member's sentence SHALL be an interactive call of their
workspace. When the workspace used its whole daily cap, compiling SHALL
answer 503 with code `ai_daily_limit_reached`, explained in the member's
language, and send nothing to the model.

#### Scenario: Asking Scout after the mail used its share
- **WHEN** a member asks Scout to compile a sentence after the day's mail reached the background limit
- **THEN** the sentence is compiled from the reserve

#### Scenario: Asking Scout past the whole cap
- **WHEN** the reserve is used too
- **THEN** the answer is 503 `ai_daily_limit_reached` and the model is not called

### Requirement: A streamed call is settled however it ends
A streamed chat call SHALL be settled once: with the usage the provider
reported when the stream ends normally, as failed when it ends in an error,
and as failed when its stream is dropped before the end (the caller hung up
or gave up), recorded on the call's span and metric as
`error.type=cancelled`. Each stream SHALL write one log line when it ends,
with its outcome, the time to its first token, the time it took, the
characters of reasoning and text received, why the model stopped (a fixed
word) and the token counts, and never any content.

#### Scenario: A finished stream
- **WHEN** a streamed call ends with usage of 120 prompt and 40 completion tokens
- **THEN** the ledger adds those tokens once to the call admitted

#### Scenario: A stream dropped early
- **WHEN** the caller drops a streamed call after its first words
- **THEN** the call is settled as failed and its span carries `error.type=cancelled`

### Requirement: A role may ask for less reasoning
`CAMPBOOKS_AI__<ROLE>__REASONING_EFFORT` (`minimal`, `low`, `medium` or
`high`, any case; blank is unset) SHALL make the role's chat calls, streamed
or not, send `reasoning: {"effort": <value>}`. Unset, the default, SHALL send
nothing. Any other value SHALL be a configuration error at startup. A role's
own effort SHALL NOT be chosen in code. A chat request MAY carry an effort of
its own, which SHALL win over the role's; it SHALL be sent only to a provider
that takes the `reasoning` object (`CAMPBOOKS_AI__<ROLE>__REASONING=true`, or
a role with a `REASONING_EFFORT` of its own) and SHALL be dropped for any
other, so a strict OpenAI-compatible server never receives the object.

#### Scenario: Unset
- **WHEN** no reasoning effort is configured for `complete`
- **THEN** its chat requests carry no `reasoning` field

#### Scenario: Low
- **WHEN** `CAMPBOOKS_AI__COMPLETE__REASONING_EFFORT=low`
- **THEN** its chat requests carry `reasoning: {"effort": "low"}`

#### Scenario: A request asks for less
- **WHEN** a request asks for low effort, the provider takes `reasoning` and the role's own effort is high
- **THEN** the request carries `reasoning: {"effort": "low"}`

#### Scenario: A provider that does not take it
- **WHEN** a request asks for low effort and the provider is not marked as taking `reasoning`
- **THEN** the request carries no `reasoning` field

### Requirement: An answer that ran out of room is asked for again once
A caller MAY ask for a chat answer with room to spare. When the answer stops
with `finish_reason=length`, no content and no tool call (the model spent its
whole budget thinking), the gateway SHALL ask once more with twice the
`max_tokens` and low reasoning effort. The second call SHALL be metered and
held to the daily cap like any call. An answer that stopped normally, wrote
something, or called a tool SHALL NOT be asked for again.

#### Scenario: Thinking used the whole budget
- **WHEN** a call returns no content with `finish_reason=length` at 4096 tokens
- **THEN** it is asked again with 8192 tokens and low effort, and that answer is returned

#### Scenario: A cut answer with words
- **WHEN** a call returns some content with `finish_reason=length`
- **THEN** it is not asked again
