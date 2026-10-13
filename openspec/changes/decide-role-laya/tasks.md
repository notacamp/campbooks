# Tasks

## 1. Typed decide API

- [x] 1.1 `decide` module: `DecisionState`, `DecisionContext`, `Question` / `QuestionKind`, `Answer` / `AnswerValue` / `Answers`, `accept_if`; verify with unit tests
- [x] 1.2 Validation (≤ 20 options/levels with a coarse-to-fine hint, unique ids and labels, ≤ 64 questions, state ≤ 50,000 chars); verify with unit tests
- [x] 1.3 Calibration: `temper` and `Calibration` lookup and parsing; verify the math (equals logit scaling, argmax kept, zeros, degenerate input) and key/value errors

## 2. Providers

- [x] 2.1 `laya`: ordered request serialization and response decoding; verify against the laya-serve 0.3.21 shapes with wiremock (exact body, auth, JSON state, malformed answers)
- [x] 2.2 `openai`: prompt, strict schema, logprob → probability mapping (shared prefixes, quote inside a larger token, multi-digit levels, UTF-8 byte tokens, non-round-tripping tokens); verify with unit and wiremock tests, with and without logprobs
- [x] 2.3 `Gateway::decide` with retries (connection errors, 5xx, `Retry-After`) and no retry on 4xx or timeouts; verify with wiremock

## 3. Config and catalog

- [x] 3.1 `protocol`, `calibration`, `logprobs` on `ProviderConfig`, decide-only validation, empty key = no key; verify old settings still parse as `openai`
- [x] 3.2 Catalog: three Laya checkpoints, Laya multilingual as default `decide`, `decide` removed from instruct models' roles; verify with unit tests

## 4. Deployment and docs

- [x] 4.1 `deploy/laya` image (pinned, CPU, non-root, preload, healthcheck, key check) and the compose `ai` profile `laya` service; verify by building it, starting it with compose, and probing it from the compose network (401 without the key, 200 with it, no published port)
- [x] 4.2 Ignored live test `crates/ai/tests/decide_live.rs` run once against the real image; latency and answers recorded in the PR and the ADR 0009 note
- [x] 4.3 `.env.example`, `deploy/README.md`, `docs/architecture.md`, ADR 0009 note, CHANGELOG
- [x] 4.4 Gates green (fmt, clippy, tests, arch, openapi check, deny, machete, OpenSpec validate); no OpenAPI or client change

## 5. Follow-ups (not in this change)

- [ ] 5.1 Store corrections per (purpose, question); refit temperatures from them; per-question eval sets
- [ ] 5.2 Per-workspace `decide` overrides and "test connection" (with the workspaces module)
- [ ] 5.3 Report the checkpoint revision per answer (laya-serve only exposes it on `/health`)
- [ ] 5.4 Fine-tune a checkpoint on our own decisions once corrections exist
