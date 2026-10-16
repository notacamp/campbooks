## ADDED Requirements

### Requirement: Every model call is attributed and metered
Every model call (`chat`, `embed`, `decide`) through a gateway composed with a
meter SHALL be attributed to a workspace and a feature, or to the instance,
before anything is sent. The server SHALL compose its gateway with the
`system` module's meter. A call with no attribution SHALL be refused with
nothing sent. Instance calls (the `decide` answers check) SHALL NOT count
against any workspace. One logical call SHALL count once, whatever the
retries after a connection failure or a 5xx answer.

#### Scenario: A call path that forgot to say whose it is
- **WHEN** code calls a metered gateway outside any usage scope
- **THEN** the call fails as unattributed and no request reaches the provider

#### Scenario: The answers check
- **WHEN** the `decide` answers check runs on a metered gateway
- **THEN** its calls are made under the instance scope and no workspace's usage changes

#### Scenario: The composed server
- **WHEN** the server composes its modules with an AI role configured
- **THEN** the gateway it hands to the modules is metered

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
role and feature. Admission SHALL be atomic, so concurrent workers and
processes never admit more than the cap. A call over the cap SHALL be
refused with nothing sent. The first refusal of a workspace's day SHALL log
one line saying that the workspace reached its daily AI limit and that its
new mail is sorted by rules only until 00:00 UTC, with the workspace id and
the cap and no content. The count SHALL start again at 00:00 UTC.

#### Scenario: Under the cap
- **WHEN** a workspace with a cap of 2 makes its first two calls of the day
- **THEN** both are sent

#### Scenario: At the cap
- **WHEN** it makes a third call that day
- **THEN** the call is refused, nothing is sent, the refusal is counted, and the limit is logged once

#### Scenario: Concurrent calls
- **WHEN** many calls of one workspace are admitted at the same time with a cap of 5
- **THEN** exactly 5 are admitted

#### Scenario: The next day
- **WHEN** the UTC day changes
- **THEN** the workspace's calls are admitted again

#### Scenario: No cap
- **WHEN** the cap is configured as 0
- **THEN** every call is admitted and still counted

### Requirement: Over the cap, mail is sorted by rules only
When the `decide` pass is refused over the cap, it SHALL store nothing and
finish without error, exactly as when no model is configured: the message
keeps what the rules decided when it was stored. A skipped message SHALL NOT
be re-queued, so the next day's cap is spent on that day's mail.

#### Scenario: The message after the cap
- **WHEN** a message arrives after its workspace's cap is reached
- **THEN** it is stored and sorted by the rules, has no model answers, and no job fails

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
