## MODIFIED Requirements

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

## ADDED Requirements

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
