# ai-decide Specification

## MODIFIED Requirements

### Requirement: Provider protocols
The `decide` role SHALL be served by the protocol named in
`CAMPBOOKS_AI__DECIDE__PROTOCOL`: `laya` or `openai`. When the setting is
absent, the protocol SHALL be `openai`, so a `decide` role configured before
this setting existed keeps its meaning. `laya` SHALL be refused at startup for
any other role, naming the variable.

- With `laya`, the gateway SHALL `POST` to `<base_url>/systemone` a body
  `{"model", "state", "questions"}` in laya-serve's format, with questions and
  options in the caller's order, and the configured key as a bearer token.
- With `openai`, the gateway SHALL `POST` to `<base_url>/chat/completions` a
  prompt holding the state and the questions, `temperature` 0, `max_tokens`
  4096 (room for a reasoning model to think before its JSON), and a strict
  JSON schema allowing only each choice's labels, each score's level numbers
  and a boolean per `noul`. Unless `CAMPBOOKS_AI__DECIDE__LOGPROBS` is false it
  SHALL request token log-probabilities and derive each answer's option
  probabilities from them; when they are missing or do not spell out the
  answer, the answer SHALL have no probabilities.
- A response that lacks an answer, picks an option the question does not have,
  or does not fit the schema SHALL fail the call as an invalid response.

#### Scenario: Laya request shape
- **WHEN** `decide` runs with `protocol = laya`
- **THEN** laya-serve receives `{"model": "<configured model>", "state": …, "questions": {"<id>": {"type": "choice", "instructions": …, "criteria": {"<label>": "<description>", …}}, …}}` with `Authorization: Bearer <key>`

#### Scenario: Existing configuration
- **WHEN** `CAMPBOOKS_AI__DECIDE__BASE_URL` and `__MODEL` are set and `__PROTOCOL` is not
- **THEN** `decide` asks that model over the OpenAI-compatible chat API

#### Scenario: Room to answer
- **WHEN** `decide` runs with `protocol = openai`
- **THEN** the request carries `max_tokens` 4096
