# ai-decide Specification

## Purpose
How Campbooks asks the `decide` AI role typed questions about an item and what
it gets back (ADR 0009): typed answers with probabilities, from a decision
model (Laya) or an instruct model, calibrated, and gated by confidence before
anything acts on them.

## Requirements

### Requirement: Typed decision call
`campbooks-ai` SHALL offer one `decide` call that takes a context naming the
decision, a state (text or JSON) and a list of questions, each with an id
unique within the call and one of three types: `choice` (one label of an
ordered set of labelled options), `score` (one level of an ordered list of
level descriptions) and `noul` (whether a statement holds). It SHALL answer
every question of the call in one provider request and return one answer per
question, in question order. An answer SHALL carry its typed value, the
probability of each option in question order when the provider can tell
(`[false, true]` for `noul`), a confidence equal to the probability of the
reported answer, the model that answered, the model version when the provider
reports one, and the latency of the call. Callers SHALL NOT receive free text
from `decide`.

#### Scenario: Three question types in one call
- **WHEN** a caller asks a `choice`, a `score` and a `noul` question about one state
- **THEN** exactly one request reaches the provider and three typed answers come back in the order asked

#### Scenario: No questions
- **WHEN** a caller asks no question
- **THEN** no request is sent and the answers are empty

### Requirement: Confidence threshold
An answer SHALL offer `accept_if(threshold)`, which returns its value only
when its confidence is known and at least the threshold. An answer without a
known confidence SHALL never pass a threshold.

#### Scenario: Below the threshold
- **WHEN** an answer's confidence is 0.7 and the caller accepts at 0.8
- **THEN** `accept_if` returns nothing and the caller asks the user instead of acting

#### Scenario: Unknown confidence
- **WHEN** an instruct model answered without log-probabilities
- **THEN** the answer has no probabilities and no confidence, and `accept_if(0.0)` returns nothing

### Requirement: Question validation
`decide` SHALL reject a call before sending anything when a `choice` has fewer
than 2 or more than 20 options, a `score` has fewer than 2 or more than 20
levels, an id, label, instruction, level or statement is empty, an id repeats
within the call, a label repeats within a question, the call has more than 64
questions, or the state is longer than 50,000 characters. The error SHALL name
the question, and for too many options SHALL suggest splitting the set into a
coarse choice followed by a fine one.

#### Scenario: Too many options
- **WHEN** a caller asks a `choice` with 30 options
- **THEN** the call fails without a request, naming the question and suggesting a coarse-to-fine split

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
  prompt holding the state and the questions, `temperature` 0, and a strict
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

### Requirement: Retries
`decide` SHALL retry a request after a connection failure or a 5xx response,
up to 3 attempts in total, waiting 250 ms and doubling up to 4 s, and waiting
at least a `Retry-After` given with a 503 (capped at the same 4 s). It
SHALL NOT retry a 4xx response or a timeout.

#### Scenario: Busy server
- **WHEN** laya-serve answers 503 with `Retry-After`, then succeeds
- **THEN** the call succeeds after the retry

#### Scenario: Rejected key
- **WHEN** laya-serve answers 401
- **THEN** the call fails at once with the status, after one request

### Requirement: Calibration
Every `decide` probability SHALL be re-tempered with a temperature `T` chosen
by question type and option count, as `pᵢ^(1/T) / Σⱼ pⱼ^(1/T)`, before the
confidence is derived. `T` SHALL come from
`CAMPBOOKS_AI__DECIDE__CALIBRATION__<TYPE>_<OPTIONS>`, else
`…__CALIBRATION__<TYPE>`, else 1.0. A non-positive or non-finite temperature,
or an unknown key, SHALL be refused at startup; so SHALL calibration settings
on any role other than `decide`.

#### Scenario: Softening an over-confident `noul`
- **WHEN** `CAMPBOOKS_AI__DECIDE__CALIBRATION__NOUL=2` and the model says P(true) = 0.9
- **THEN** the answer's probability of true and its confidence are 0.75

### Requirement: Privacy of the state
The gateway SHALL NOT log the state of a decision, and the state's debug
representation SHALL show only its length.

#### Scenario: Debug output
- **WHEN** a state is formatted with `{:?}`
- **THEN** the output shows its kind and length, not its content

### Requirement: Default model
The model catalog SHALL list Laya multilingual (Apache-2.0) as the default
`decide` model and the English and typed-decisions Laya checkpoints as
alternatives. Instruct models SHALL NOT list `decide` among their suggested
roles; they remain usable for `decide` over `openai`.

#### Scenario: Default lookup
- **WHEN** a setup screen asks the catalog for the default `decide` model
- **THEN** it gets `convaiinnovations/laya-multilingual`, served over the `laya` protocol

### Requirement: Self-hosted decision model
The compose `ai` profile SHALL run a `laya` service built from
`deploy/laya/Dockerfile` with pinned versions, as a non-root user, on CPU,
with the multilingual checkpoint kept in a volume and loaded at startup, and a
healthcheck. The service SHALL publish no port and SHALL refuse to start when
`CAMPBOOKS_AI__DECIDE__API_KEY` is empty, saying how to set it.
`.env.example` SHALL point `decide` at it.

#### Scenario: No key
- **WHEN** the `ai` profile starts and `CAMPBOOKS_AI__DECIDE__API_KEY` is empty
- **THEN** the `laya` container exits with a message telling the operator to set the key

#### Scenario: Request without the key
- **WHEN** a container on the compose network calls the `laya` service without the bearer key
- **THEN** it gets 401
