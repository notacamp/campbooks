# Tasks

## 1. Decide
- [x] 1.1 `money` choice question (owed / paid / neither) in `triage_questions`
- [x] 1.2 Answers check: a bill and a payment confirmation, money expectations on six cases (50 answers); test responders answer `money`
- [x] 1.3 Policy: `money` records until owner-labelled data; pinning test covers unmeasured questions

## 2. Needs
- [x] 2.1 `raise()`: Pay only for `owed`, as a question; nothing else in Receipts; list mail never
- [x] 2.2 Verdicts `already_paid` / `still_to_pay`; settle and withdrawal; `resolve_need` outcomes, `not_a_payment` (four locales); money corrections
- [x] 2.3 Migration: widened checks (validated separately), every inbox queued for the backfill
- [x] 2.4 Backfill and backlog ask again for messages missing a triage answer
- [x] 2.5 Tests: unit (raise, settle), API (Is it paid?, Not needed, only a bill), fixtures answer `money`

## 3. Labelling
- [x] 3.1 `campbooks-server labelling export` (manifest, items, labels.csv) with tests
- [x] 3.2 `cargo xtask eval decide --set-dir`, optional labels, `labels.csv`, scored questions from the manifest

## 4. Home (on campbooks#515)
- [x] 4.1 Pay question row, Already paid (1) / Still to pay (2), All N already paid with one Undo
- [x] 4.2 Still to pay: "Pay <payee>", due/overdue from `due_on`, reminder first action
- [x] 4.3 "How Scout chose these": the money answer
- [x] 4.4 Recording scenario (`pay-question`); the reader's bill choice

## 5. Last
- [x] 5.1 OpenAPI and client types regenerated
- [x] 5.2 CHANGELOG updated
- [ ] 5.3 Gates green on CI; UI verified at 375px and desktop
