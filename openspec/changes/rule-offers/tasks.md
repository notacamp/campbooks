## 1. Engine and rules (`campbooks-ai`, `conversations`)

- [x] 1.1 Condition `sender_checks_fail` (facts: the receiving provider's authentication results); actions `trash`, `spam`, `mark_paid`; slots `bin`, `money`; guards; tests
- [x] 1.2 Apply on arrival and over existing mail, with undo (`before.binned`), exceptions on untrash / not spam
- [x] 1.3 `mark_paid` answers `money: paid` (taught)
- [x] 1.4 `suggest_rules` `action=report|paid`; tests

## 2. Scout noticed (`conversations`)

- [x] 2.1 Migration: `rule_notices`, `rule_notice_answers`, `rule_notice_scans` (forced RLS)
- [x] 2.2 Analysis over counts and facts (pure, tested); settings with defaults
- [x] 2.3 `conversations.notice_rules` pass; `list_rule_notices` (starts stale passes); answers with undo
- [x] 2.4 `create_rule.notice_id`; Undo offers it again; purges
- [x] 2.5 Tests: notice, answers per member, access (stranger, reader, other workspace, other inbox)

## 3. Client

- [x] 3.1 `SmartRuleConfirm` (dialog, on `lib/ui`'s `Dialog` and the `RulePreview` card tagged "Smart rule") and tests
- [x] 3.2 Teach once's accept opens the confirmation
- [x] 3.3 Report and delete on a warning; the offer after it
- [x] 3.4 The offer after Already paid
- [x] 3.5 "Scout noticed" at the bottom of Home
- [x] 3.6 Rule sentences for the new condition and actions; strings in en, pt-PT, es-ES, fr

## 4. Finish

- [x] 4.1 OpenAPI, events and client types regenerated; sqlx offline cache
- [x] 4.2 Recording scenario (375px and desktop)
- [x] 4.3 CHANGELOG
- [ ] 4.4 Gates green in CI; review agent; archive the change
