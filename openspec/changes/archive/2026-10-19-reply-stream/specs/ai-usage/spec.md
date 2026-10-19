## MODIFIED Requirements

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

## ADDED Requirements

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
nothing. Any other value SHALL be a configuration error at startup. No
effort SHALL be chosen in code.

#### Scenario: Unset
- **WHEN** no reasoning effort is configured for `complete`
- **THEN** its chat requests carry no `reasoning` field

#### Scenario: Low
- **WHEN** `CAMPBOOKS_AI__COMPLETE__REASONING_EFFORT=low`
- **THEN** its chat requests carry `reasoning: {"effort": "low"}`
