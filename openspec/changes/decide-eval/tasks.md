# Tasks

## 1. Eval set

- [ ] 1.1 Manifest (version, questions, label sets, target precision) and labelling guide; verify the guide covers every hard-case tag
- [ ] 1.2 Draft ~80 items per language (`en`, `pt-PT`, `es-ES`, `fr`) to target label vectors with the hard-case and header-less quotas; verify counts per stream, language and split
- [ ] 1.3 Blind second labelling pass, adjudication, rulings added to the guide; verify agreement per question is recorded and spot-check a sample per language
- [ ] 1.4 Dataset test in `cargo test`: schema, unique ids, reserved domains only, consistency rules; verify it fails on a planted bad item

## 2. Strategies (`campbooks-ai::decide::strategy`)

- [ ] 2.1 `Envelope`, `decision_state()`, `Strategy` and `Corrections` traits; verify with unit tests
- [ ] 2.2 `rules` strategy (headers, attachments, sender patterns, per-language keywords and dates, look-alike domains); verify per rule with unit tests
- [ ] 2.3 `knn` with `LexicalEmbedder` and `GatewayEmbedder`; verify neighbour voting, abstention and the embed call with wiremock
- [ ] 2.4 `model` strategy over `Gateway::decide` (Laya and instruct question wording); verify requests and answers with wiremock
- [ ] 2.5 `cascade`; verify stage order, thresholds and that a fully decided item makes no model call

## 3. Harness (`cargo xtask eval decide`)

- [ ] 3.1 Metrics: accuracy, macro-F1, positive recall, MAE, coverage/precision at τ, operating point, Wilson interval, ECE, Brier, temperature fit, latency; verify each against hand-computed fixtures
- [ ] 3.2 CLI, env-driven strategy construction, "not run" reporting, markdown and JSON report; verify with a run on the set
- [ ] 3.3 `--offline --check` against the committed baseline; add it to CI

## 4. Results and docs

- [ ] 4.1 Run every strategy that can run locally; commit `reports/baseline.{md,json}`; record numbers in the PR
- [ ] 4.2 ADR amending 0009 with the per-question policy once a model strategy has been run (otherwise a `decisions-needed.md` entry)
- [ ] 4.3 CHANGELOG; no OpenAPI or client change; gates green (fmt, clippy, tests, arch, openapi check, deny, machete, OpenSpec validate)
