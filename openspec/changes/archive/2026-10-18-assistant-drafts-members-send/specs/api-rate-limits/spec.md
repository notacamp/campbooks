## MODIFIED Requirements

### Requirement: Automation and the assistant are limited separately
Operations invoked in-process by automation or by the assistant on behalf of
a person SHALL be counted per (person, caller kind) at the same read and
write limits, apart from the person's own apps, so a looping rule or
assistant is stopped without refusing the person's own requests. Neither
SHALL send: `conversations.send_draft` opts neither in, so an invoked send
is refused before it is counted, and the person's send budget is theirs
alone.

#### Scenario: A looping rule
- **WHEN** automation invokes reads for a person beyond the read limit
- **THEN** the invocation fails with 429 `rate_limited`, while the person's own reads through the web app still succeed

#### Scenario: A send by the assistant
- **WHEN** the assistant or automation invokes `conversations.send_draft` for a person, as a call marked confirmed
- **THEN** the invocation is refused before the handler runs, and the person's own sends through the web app still have their whole budget
