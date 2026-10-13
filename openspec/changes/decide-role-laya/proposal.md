# Proposal

## Why

ADR 0009 makes `decide` a typed decision protocol instead of a chat call:
callers ask `choice` / `score` / `noul` questions about a compact state and get
answers with probabilities they can gate on. Today `campbooks-ai` only has the
OpenAI-compatible chat and embeddings calls; `decide` is "a chat model with a
JSON schema" and every caller would parse text and get no usable probability.
The first modules that decide things about items (conversations, tasks,
reconciliation) need the typed call, the default model behind it and a way to
run that model in the self-hosted stack.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/58
(sub-task of #20, the AI gateway).

## What Changes

- `campbooks-ai` gains `Gateway::decide(ctx, state, questions) -> Answers`:
  typed questions (`Choice` with ordered options, `Score` with ordered levels,
  `Noul` with a statement) and typed answers (value, a probability per option,
  confidence, model, model version, latency), plus `Answer::accept_if`.
  Questions are validated before any request: at most 20 options per choice or
  levels per score (the error suggests a coarse-to-fine split), unique ids and
  labels, at most 64 questions, a state of at most 50,000 characters.
- Two providers behind the `decide` role, picked by the new
  `CAMPBOOKS_AI__DECIDE__PROTOCOL`:
  - `laya`: `POST <base>/systemone` on `laya-serve`, bearer key, retries with
    backoff on connection errors and 5xx (honouring `Retry-After`), timeouts;
  - `openai` (the default, so existing settings keep their meaning): an
    instruct model answering the same questions with strict JSON-schema output;
    probabilities from token logprobs when the server returns them
    (`CAMPBOOKS_AI__DECIDE__LOGPROBS`, default true), otherwise none.
- Calibration: a temperature per (question type, option count) applied to
  every `decide` probability, from `CAMPBOOKS_AI__DECIDE__CALIBRATION__*`
  (default 1.0). Later refitted from user corrections.
- Catalog: Laya multilingual (Apache-2.0) is the default `decide` model; the
  English and typed-decisions checkpoints are listed; instruct models no longer
  list `decide` among their default roles.
- `deploy/`: a `laya` service in the compose `ai` profile, built from
  `deploy/laya/Dockerfile` (pinned Python, CPU PyTorch and `laya[serve]`,
  non-root, checkpoint preloaded into a `laya-models` volume, healthcheck),
  reachable only on the compose network, refusing to start without an API key.
  `.env.example` points `decide` at it.
- An empty `CAMPBOOKS_AI__<ROLE>__API_KEY` now means "no key".

## Capabilities

### New Capabilities

- `ai-decide`: how Campbooks asks typed decision questions and what it gets
  back, which providers can serve them, and how the self-hosted stack runs the
  default one.

### Modified Capabilities

None. `embed`, `complete` and `vision` are unchanged.

## Impact

- Crates: `campbooks-ai` only (config, catalog, gateway, new `decide` module).
  No module, no endpoint, no migration, no OpenAPI change, no new
  dependency (`tokio` moves from dev to normal dependencies of
  `campbooks-ai` for the retry sleep; it is already in the workspace).
- Self-hosters: new optional settings (`…DECIDE__PROTOCOL`, `…__LOGPROBS`,
  `…__CALIBRATION__*`); `.env.example` now points `decide` at the `laya`
  service and needs `CAMPBOOKS_AI__DECIDE__API_KEY` for the `ai` profile.
  Existing `decide` settings without a protocol keep working as `openai`.
- Images: the Campbooks image is unchanged. The `laya` image is optional,
  built locally, and not part of the release pipeline or its Trivy scan.

## Non-goals

- Any module calling `decide`. The first callers (conversations triage,
  tasks) come with their modules.
- Per-workspace `decide` overrides and the "test connection" action from ADR
  0005: they need the workspaces module and its encrypted settings.
- Storing corrections, refitting temperatures from them and per-question eval
  sets: they need the first decisions to correct. This change only provides
  the calibration hook and its configuration.
- Fine-tuning Laya on our own decisions.
- `max_len` / `head_max_len` overrides for long states (states are meant to be
  compact summaries; ADR 0009).
- A published, scanned Laya image.
