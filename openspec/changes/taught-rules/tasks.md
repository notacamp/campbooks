## 0. Design gate

- [x] 0.1 Proposal, design with alternatives, spec deltas, visual prototype (https://claude.ai/artifact/2y7z5yvKsMpKC6nVTsPrw7)
- [x] 0.2 Owner direction: one shared, deterministic rules system; don't wait for approval (open questions take their recommended defaults)
- [x] 0.3 Shared rule model handed to #121 (compiles language and onboarding answers into these operations)
- [x] 0.4 Split agreed with the coordinator: this change is the backend (PR A); the client is the stacked change `teach-once-ui` (PR B)

## 1. Engine (`campbooks-ai`)

- [ ] 1.1 `signals::email::subject_template` (prefixes in four locales, dates, digit runs, ids, two literal words); unit tests in en, pt-PT, es-ES, fr
- [ ] 1.2 `decide::taught`: `Condition`, `Action`, `TaughtRule`, `Facts`, `evaluate` (all conditions, per-slot winner by specificity → confirmed → id, labels additive, suspicious guard, `surface: always` guard); unit tests per precedence row
- [ ] 1.3 `TaughtRules` as a `Strategy` for `stream`, `needs_reply`, `asks_action` (source `taught`); `cargo xtask eval decide` unchanged

## 2. Storage and operations (`conversations`)

- [ ] 2.1 Migration `conversations__taught_rules`: `rules`, `rule_runs`, `rule_applications`, `rule_dismissals` with forced RLS; `messages.subject_template` and indexes; `taught` in `stream_source` and `decisions.source`; `rule` in `needs.cleared_because`
- [ ] 2.2 Migration `attention__rule_cause`: `rule` in `attention.items.cleared_because`; attention keeps rule-cleared items out of Done today; test
- [ ] 2.3 `subject_template` at ingest; backfill job `conversations.backfill_subject_templates`; test
- [ ] 2.4 `domain::rules`: validation (`invalid_rule`), likeness → SQL match, preview (count, five samples, skipped, overrides); tests
- [ ] 2.5 Operations `preview_rule`, `create_rule`, `confirm_rule`, `update_rule`, `remove_rule`, `list_rules`, `get_rule` with roles (`inbox_read_only`, `rule_not_yours`, 404), `x-campbooks` undo, catalogue errors in four locales; `rule_changed` v1 with contract and feed declaration; tests
- [ ] 2.6 Pause a member's rules when they lose `sender` on the inbox; adopt by a manager; purge with the inbox and workspace; tests

## 3. Rules in the decide pass

- [ ] 3.1 `apply_changes`: evaluate active rules per new inbound message; taught decisions; stream source `taught`; archive / read / label on arrival; applications recorded; tests (new notification, specificity, suspicious guard, rule beats signals)
- [ ] 3.2 Needs: taught answers in the derivation; `surface` never and always; cause `rule`; tests
- [ ] 3.3 Exceptions in `move_to_stream`, `unarchive`, `mark_unread`, `remove_label`, `reopen_need`; `rule_exception` in their responses; `rule` in conversation summaries; tests

## 4. Runs and suggestions

- [ ] 4.1 `conversations.apply_rule` job: batches of 100, cursor, idempotent applications, re-checks, events, progress; resume test; 1,000-conversation test
- [ ] 4.2 Revert run for `remove_rule?restore=true` (skips excepted and member-changed); cancel a running apply; tests
- [ ] 4.3 `suggest_rules` (likenesses broad → narrow, default by machine sender / List-Id / template, correspondents, thresholds) and `dismiss_rule_suggestion` (30 days, then for good); tests
- [ ] 4.4 Terminal recording (vhs): create a rule over 1,000 demo conversations, watch progress, a new message handled on arrival, undo all

## 5. Finish

- [x] 5.1 `docs/domain-map.md`: taught rule, likeness, run, application, exception in conversations' language
- [ ] 5.2 OpenAPI regenerated, client types regenerated, `openapi/events.json` regenerated
- [ ] 5.3 CHANGELOG updated
- [ ] 5.4 Gates green; migration duplicate check against `next`
- [ ] 5.5 Archive the change into `openspec/specs/`
