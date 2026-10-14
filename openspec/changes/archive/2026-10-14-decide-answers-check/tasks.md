# Tasks

## 1. Investigation (campbooks-cloud#128)

- [x] 1.1 Compare the request, the answer mapping and the serving setup with laya-serve 0.3.21's source and the model card; verify prod loads the published multilingual weights (sha256) and that a local laya-serve with the same pins answers like prod
- [x] 1.2 Probe prod's Laya with fictional emails only (no restart, no configuration or database change): today's requests, negated statements, swapped option texts, yes/no questions, a two-option choice, JSON state, context; verify the mapping is not inverted and the model ignores the question
- [x] 1.3 Run `cargo xtask eval decide --strategy laya` against a local laya-serve with prod's pins, plus the wording variants on half the set; record AUROC per question

## 2. Context and questions (`campbooks-ai`)

- [x] 2.1 `decide::signals::inbox`: `Recipient`, the hints, `preamble`, `CONTEXT_VERSION`; verify with a golden test, the shared inbox, a missing recipient and a header that tries to break out
- [x] 2.2 `Envelope.recipient`, `decision_state()` opens with the context; yes/no wording for the four yes/no triage questions
- [x] 2.3 Measure the context's token budget with the multilingual tokenizer over the eval set

## 3. Answers check (`campbooks-ai`)

- [x] 3.1 `decide::check`: the eight cases, grading, outcome, stamp; verify both answers per yes/no question and the 90% line
- [x] 3.2 `Gateway::{run_answers_check, keep_answers_checked, answers_check, answers_trusted, record_answers_check}` and `RoleHealth.answers_check`; `Model::checked()` and `Error::AnswersNotTrusted`
- [x] 3.3 Recorded laya-serve responses as fixtures; verify they fail the check (8 and 18 of 33), map to what laya-serve said, that a checked model refuses before calling, and that a model getting them right passes and stamps its answers

## 4. Server, conversations, system, client

- [x] 4.1 The server spawns `keep_answers_checked` in every process
- [x] 4.2 `conversations.decide_message`: checked model, recipient from the inbox, skip on `AnswersNotTrusted`; verify a failed check sorts exactly like no model, and the stamp and envelope version 2
- [x] 4.3 Migration marking earlier model answers `laya-0.3.21-unchecked`/`unchecked`; read-side filters in suggestions and needs; verify unchecked answers are kept but not read and raise no need
- [x] 4.4 `AiAnswersCheck` in the admin overview; the System panel shows it in four languages; verify with component tests
- [x] 4.5 Eval items get their owner as the recipient

## 5. Wrap-up

- [x] 5.1 OpenAPI regenerated, client types regenerated, `.sqlx` regenerated
- [x] 5.2 After-change eval with Laya recorded in the PR (expected: still failing; the model choice is campbooks-cloud#128)
- [x] 5.3 CHANGELOG updated
- [x] 5.4 Gates green; recordings: terminal (the check failing on Laya, before and after) and the System panel at 375px and desktop
