# Tasks

## 1. Eval set

- [x] 1.1 Manifest (version, questions, label sets, target precision) and labelling guide; verify the guide covers every hard-case tag
- [x] 1.2 Draft ~80 items per language (`en`, `pt-PT`, `es-ES`, `fr`) to target label vectors with the hard-case and header-less quotas; verify counts per stream, language and split
- [x] 1.3 Blind second labelling pass, adjudication, rulings added to the guide; verify agreement per question is recorded and spot-check a sample per language
- [x] 1.4 Dataset test in `cargo test`: schema, unique ids, reserved domains only, consistency rules; verify it fails on a planted bad item

## 2. Strategies (`campbooks-ai::decide::strategy`)

- [x] 2.1 `Envelope`, `decision_state()`, `Strategy` and `Corrections` traits; verify with unit tests
- [x] 2.2 `signals` strategy (`decide::signals::email`) (headers, attachments, sender patterns, per-language keywords and dates, look-alike domains); verify per rule with unit tests
- [x] 2.3 `knn` with `LexicalEmbedder` and `GatewayEmbedder`; verify neighbour voting, abstention and the embed call with wiremock
- [x] 2.4 `model` strategy over `Gateway::decide` (Laya and instruct question wording); verify requests and answers with wiremock
- [x] 2.5 `cascade`; verify stage order, thresholds and that a fully decided item makes no model call

## 3. Harness (`cargo xtask eval decide`)

- [x] 3.1 Metrics: accuracy, macro-F1, positive recall, MAE, coverage/precision at τ, operating point, Wilson interval, ECE, Brier, temperature fit, latency; verify each against hand-computed fixtures
- [x] 3.2 CLI, env-driven strategy construction, "not run" reporting, markdown and JSON report; verify with a run on the set
- [x] 3.3 `--offline --check` against the committed baseline; add it to CI

## 4. Results and docs

- [x] 4.1 Run every strategy that can run locally; commit `reports/baseline.{md,json}`; record numbers in the PR
- [x] 4.2 No model endpoint was available, so the ADR amending 0009 waits: `decisions-needed.md` entry "Model endpoints for decide evals"
- [x] 4.3 CHANGELOG; no OpenAPI or client change; gates green (fmt, clippy, tests, arch, openapi check, deny, machete, OpenSpec validate)

## 5. Follow-ups (not in this change)

- [ ] 5.1 Run Laya, an instruct model and `knn-embed` on the set once endpoints exist; write the ADR amending 0009 with the strategy and threshold per question
- [ ] 5.2 Conversations (campbooks-cloud#37) calls `EmailSignals` / `Cascade` and stores corrections as `Neighbours`
- [ ] 5.3 The same harness on a private, opt-in export of real corrections
