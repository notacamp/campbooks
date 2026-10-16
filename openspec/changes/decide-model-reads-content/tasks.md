# Tasks

## 1. Decisions and rules
- [x] 1.1 ADR 0018 (amends 0005, 0009); AGENTS.md: the no-content-rules rule with its enforcement, and the open-weight line pointing to ADR 0018
- [x] 1.2 `signals/facts.rs`: `Facts::of`, `EmailFacts` (needs_reply false only: list mail, automatic, no-reply sender), hints; source test that the module reads no subject, body or attachment name
- [x] 1.3 `signals/email.rs`: delete every content regex, phrase list, `Read`, `conf`, quoted sentences, security platforms; delete `signals/dates.rs`; keep `Envelope`, triage questions, `decision_state`, `embedding_text`
- [x] 1.4 `inbox::preamble` hints from facts; `CONTEXT_VERSION` 2

## 2. Provider and eval
- [x] 2.1 `CAMPBOOKS_AI__DECIDE__PATH` (decide only; defaults per protocol); gateway, health unchanged; tests for both paths
- [x] 2.2 xtask eval: `signals` renamed `facts`; `jev` strategy (`CAMPBOOKS_EVAL_JEV_*`); `--offline --check` ignores online runs in the baseline
- [x] 2.3 Regenerate `reports/baseline.{json,md}` with `facts` and `jev`

## 3. Conversations
- [x] 3.1 `Policy::baseline` from the report (facts needs_reply; model per question); pinning test reads both runs
- [x] 3.2 Model pass places the stream (shared `sort_first_inbound` in `domain/`), stream source `model`, member moves stick
- [x] 3.3 `needs::raise`: pay = accepted asks_action on accepted `receipts`; no due date; remove `PAYMENT_DUE`, `DEADLINE`, `resolve_date`
- [x] 3.4 Migration: old rules answers not acted; every inbox queued for the needs backfill
- [x] 3.5 Backfill asks the model about the newest inbound message without a checked model answer (ids-only job, idempotent)
- [x] 3.6 Tests and fixtures rewritten to the new rules: facts veto beats model yes; notifications with "?" raise nothing; model sorts; member move sticks; pay vs do; upgrade withdraws old needs

## 4. Client and docs
- [x] 4.1 Reason copy `facts.list_mail`, `facts.automatic`, `facts.no_reply_sender` in en, pt-PT, es-ES, fr; old `signals.*` keys kept
- [x] 4.2 `deploy/.env.example` and `deploy/README.md`: Jev through OpenRouter (PATH, MODEL, EXTERNAL_CONTENT, where mail goes)
- [x] 4.3 OpenAPI regenerated, client types regenerated, CHANGELOG updated, gates green
- [ ] 4.4 Terminal recording of the eval and the decide pass (no UI change)
