## 1. Catalogue and questions (`campbooks-ai`)

- [x] 1.1 `signals::actions`: `ActionId`, `action_questions` (facts veto), `likelihoods`, `COMBOS`; tests (vetoes, pairs, slots)
- [x] 1.2 `max_tokens` 4096 on the OpenAI-compatible decide path; test

## 2. Conversations

- [x] 2.1 `ActionSettings` (`ENABLED` off, `FLOOR_PERCENT` 70)
- [x] 2.2 Migration: `action_likelihoods` (forced RLS, cascades with the message)
- [x] 2.3 The model pass asks the catalogue with triage when on; stores ratings; triage recorded as before
- [x] 2.4 `NeedView.actions` from `rank`; absent when nothing rated
- [x] 2.5 Tests: off asks exactly triage and every need is as before; on changes no triage answer; ranking

## 3. Owner tooling

- [x] 3.1 `labelling export --set actions` (same guards as money)
- [x] 3.2 `labels.csv` action columns read by the eval
- [x] 3.3 `cargo xtask eval actions` (precision/recall per action per floor, pairs done together)

## 4. Finish

- [x] 4.1 OpenAPI and client types regenerated
- [x] 4.2 CHANGELOG
- [x] 4.3 CI green; review; archive
