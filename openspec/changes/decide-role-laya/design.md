# Design

## Context

`campbooks-ai` has a `Gateway` with `chat` and `embed` over the
OpenAI-compatible API, an `AiConfig` mapping each role to a `ProviderConfig`
(`base_url`, `api_key`, `model`) from `CAMPBOOKS_AI__<ROLE>__*`, and a static
catalog of open-weight models. ADR 0009 settles the `decide` protocol and Laya
as its default. Laya is served by the `laya` Python package's `laya-serve`
(FastAPI); there is no official container image. The request and response
shapes below were read from the `laya` 0.3.21 source (`laya/serve.py`,
`laya/agent.py`, `laya/router.py`) and checked against a running server.

## Data ownership and events

- No Postgres schema, table or migration. `campbooks-ai` owns no data.
- No events published or consumed.
- No endpoint, so no OAuth scope and no OpenAPI change.
- **AI:** this change *is* the `decide` role. When the role is not configured,
  `decide` fails with `Error::RoleNotConfigured(Decide)` (a 503 through
  `kernel::Error`) without a request; callers degrade by not deciding. Nothing
  here changes user data, so there is nothing to undo; callers that act on an
  answer do so as undoable proposals (AGENTS.md).

## API

```rust
let answers = gateway
    .decide(&DecisionContext::new("mail.triage"), &state, &[
        Question::choice("stream", "Which stream does this message belong to?", options),
        Question::score("urgency", "How soon does it need a reply?", ["no reply", "this week", "today"]),
        Question::noul("asks_user", "The sender asks the recipient to do something."),
    ])
    .await?;
if let Some(AnswerValue::Choice(stream)) = answers.get("stream").and_then(|a| a.accept_if(0.9)) { … }
```

- `DecisionState` is text or JSON; its `Debug` prints only a length, and the
  gateway never logs it (only purpose, protocol, model, question count and
  latency).
- `DecisionContext { purpose }` names the decision (`<module>.<decision>`) in
  traces; corrections, evals and refitted temperatures will be filed under it.
- `Answer.probabilities` is aligned with the question: choice options in
  order, score levels, `[false, true]` for noul. `confidence` is the largest
  probability, i.e. the probability of the reported answer (Laya's
  `answer_confidence`, the quantity temperature scaling fits), so one threshold
  means the same across question types. A score's value is the expected level
  when probabilities are known.
- `accept_if(threshold)` returns the value only when confidence is known and at
  least the threshold.

## Providers

### `laya`

`POST {base_url}/systemone` with `Authorization: Bearer <key>`:

```json
{"model": "convaiinnovations/laya-multilingual",
 "state": "…" ,
 "questions": {"stream": {"type": "choice", "instructions": "…", "criteria": {"label": "description"}},
               "urgency": {"type": "score", "instructions": "…", "criteria": ["level 0", "level 1"]},
               "asks_user": {"type": "noul", "instructions": "<statement>"}}}
```

Questions and criteria are serialized in the caller's order: option order is
positional for the model (a `serde_json::Value` would sort keys). `model`
accepts the Hugging Face id or a checkpoint name; naming it explicitly stops
laya-serve's language router from sending mostly-English text to the English
checkpoint. The response's `answers.<id>` carries `choice` + `probabilities`
by label, `score` + `probabilities` by level index, or `noul` (P(true));
`routing.repo` names the checkpoint that answered (the top-level `model` is a
fixed agent name). laya-serve does not report a checkpoint revision per
request, so `model_version` is `None` for Laya (its `/health` has it).

### `openai`

`POST {base_url}/chat/completions` with a fixed system prompt, the state and
questions in the user message, `temperature: 0`, a strict JSON schema (choice
→ string enum of labels, score → integer enum of level indices, noul →
boolean, all required, no extra properties) and, unless
`CAMPBOOKS_AI__DECIDE__LOGPROBS=false`, `logprobs: true, top_logprobs: 20`.
Model version is `system_fingerprint` when present.

Probabilities from logprobs: the tokens (by `bytes` when given) must spell the
content exactly, else no probabilities. For each answer, find where its value
starts after its key; walk the chosen tokens from there, following every
option that is still a prefix-match; at each token, credit every top
alternative to the options it begins (shared evenly if several) at the
probability of the path so far; an option completes when the token covers its
whole text (strings end with their quote, numbers and literals need a
non-alphanumeric next character, so `1` does not absorb `10`). The credited
mass is renormalised; mass on tokens that begin no option is dropped. These
are estimates for gating, recalibrated by the same temperatures.

### Retries and timeouts

Only `decide` retries (chat and embed are unchanged): connection errors and
5xx answers, up to 3 attempts, 250 ms doubling to 4 s, a 503's
`Retry-After` respected up to the cap. Timeouts (the gateway's request
timeout, 2 min) are not retried: on a slow CPU host a retry only lengthens the
queue. 4xx answers are returned at once.

## Calibration

`temper(p, T) = p^(1/T) / Σ p^(1/T)`, which equals dividing the logits by `T`
and never changes the argmax. `Calibration::temperature(type, n)` looks up
`<type>_<n>`, then `<type>`, then 1.0. Configuration:
`CAMPBOOKS_AI__DECIDE__CALIBRATION__CHOICE_2=1.9`, `…__NOUL=2.0` (figment
lower-cases the keys). Keys and values are validated at startup (positive,
finite; known types). The multilingual Laya checkpoint ships with no fitted
temperature (all 1.0), so this hook is where calibration happens. A later
change stores corrections and refits per (purpose, question).

## Config compatibility

`ProviderConfig` gains `protocol` (default `openai`), `calibration` (default
empty) and `logprobs` (default true). A `decide` configured before this change
keeps working as an instruct model. `protocol = laya` or a calibration on
another role is a startup error naming the variable. An empty `API_KEY`
becomes `None`, so `.env.example` can ship the key line blank.

## Deployment

`deploy/laya/Dockerfile`: `python:3.12.14-slim-bookworm`, `torch==2.14.0`
from the PyTorch CPU index (no CUDA wheels), then `requirements.txt` pinning
`laya[serve]==0.3.21` and its libraries (each at least two weeks old;
`huggingface_hub` stays on 1.x because `transformers` 5.17 requires it),
`USE_TF=0`, user 10001, `HF_HOME=/models`, `LAYA_MODELS=multilingual`,
`LAYA_PRELOAD=1`, `LAYA_REVISION=reviewed` (the checkpoint commit the package
pins as reviewed), a Python `/health` healthcheck with a 10-minute start
period for the first download. The entrypoint exits with a message when
`LAYA_API_KEY` is empty.

Compose `ai` profile: service `laya`, `build: ./laya`, no `ports`, volume
`laya-models`, `LAYA_API_KEY: ${CAMPBOOKS_AI__DECIDE__API_KEY:-}`.

**Key: required from `.env`, not generated.** Generating it on first boot
(like the IdP's secrets) would need the server to read it from a shared volume
(a new `API_KEY_FILE` setting) and to wait for the laya container. One value in
`.env`, read by the server and interpolated into the laya service, is simpler.
A compose-level `${VAR:?}` cannot be used: older compose versions evaluate it
even for services outside the active profiles, which would break
`docker compose up` without AI. The entrypoint check gives the error instead.

## Risks

- **Zero-shot quality.** On a sample invoice email the multilingual checkpoint
  answered the English category correctly (0.99) but the Portuguese one wrong
  at 0.9997, and the `noul` "asks the recipient to do something" wrong at
  P(true) ≈ 0.003 in both languages. The model card reports base checkpoints
  near chance on its typed-decisions benchmark zero-shot. High confidence is
  therefore not evidence of a right answer until a question has an eval set;
  see the note on ADR 0009.
- laya-serve's wire format is not versioned; the request/response shape is
  pinned by the package version in the image and covered by mock tests.
