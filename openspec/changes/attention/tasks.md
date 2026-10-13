## 0. Design gate

- [x] 0.1 Proposal, design with alternatives, spec deltas, visual prototype
- [x] 0.2 Owner sign-off on the design and the prototype (archive vs Do/Pay, phishing warnings, Done in shared inboxes)

## 1. Email signals (`campbooks-ai`)

- [x] 1.1 `Reason.sentence`: the sentence of the new text that holds the evidence (≤ 240 chars, cut at a word); unit tests in en, pt-PT, es-ES, fr, quoted history excluded
- [x] 1.2 `resolve_date(evidence, sent_on)`: weekdays, today/tomorrow, end of week/month, numeric day-first, unresolved → `None`; unit tests
- [x] 1.3 `cargo xtask eval decide` unchanged (answers identical before and after)

## 2. Conversations: needs

- [x] 2.1 Migration `conversations__needs`: `conversations.needs` with RLS; `reasons` keep `sentence`
- [x] 2.2 `domain::needs::derive` (pure): precedence, trust, window, clearing table; unit tests per row of the design's table
- [x] 2.3 `Policy`: the 0.8 floor for Review questions from the model; test
- [x] 2.4 Re-derive in `apply_changes`, `decide_message` and the tidying operations; publish `conversations.need_changed` v1 with its contract and a growing revision; tests (replied from another client, archive keeps Pay, trash clears all, old mail raises nothing, rules only never `check`)
- [x] 2.5 `resolve_need` / `reopen_need` / `confirm_need` with corrections (withdrawn on reopen), roles, 409s, `x-campbooks` undo; tests
- [x] 2.6 `ConversationSummary.need` and `conversations.list?ids=`; tests (an unseen id is left out, 1–100 else 400)

## 3. Attention module

- [x] 3.1 `cargo xtask new-module attention`; schema `attention` (`items`, `snoozes`) with RLS; scopes `attention:read`, `attention:write`
- [x] 3.2 `attention.project_conversation_need`: upsert by record, newer revisions only, snoozes ended by a new cause; replay test
- [x] 3.3 `attention.list` (open and cleared today; visibility through the `Visibility` port; order; counts; 7 days past due left out); tests
- [x] 3.4 `attention.snooze` / `attention.unsnooze` (`invalid_until`, 404 for unseen items); tests
- [x] 3.5 `attention.item_changed` within the container; feed test (a member outside the inbox gets nothing)
- [x] 3.6 `workspace_deleting` and `member_left` purges; tests
- [ ] 3.7 Terminal recording (vhs): a demo message raises a need, list, Done, reopen, snooze

## 4. Client: inbox

- [x] 4.1 `lib/ui`: need chip on `ConversationRow`; `Note` with a lead sentence, a quote and "How Campbooks decided" (replacing the percentage); stories and tests
- [x] 4.2 `conversations` module: chips on rows, the reader's need, Done (E), Not needed (X), Yes/No (1/2), undo; templates in en, pt-PT, es-ES, fr

## 5. Client: Home

- [x] 5.1 `attention` client module: queries keyed by operationId; rows rendered from the conversations module's batch read (`conversations.list?ids=`, its public helpers); Later on the registry's L, Not needed on X (`conversations.resolve_need`)
- [x] 5.2 Home: groups, hero, item detail with "How Campbooks decided", Later picker, Done today, empty state; replaces the empty Home once an inbox is visible
- [x] 5.3 Live: feed invalidation, refetch at the earliest snooze end, tab bar badge
- [x] 5.4 i18n en, pt-PT, es-ES, fr; screen stories for every state; module tests
- [x] 5.5 Fake Google demo mailbox: a question with a date, a reply owed, a lease to sign, a bill, a phishing message; `pnpm record home` scenario

## 6. Wrap-up

- [x] 6.1 OpenAPI regenerated, `openapi/events.json` regenerated, client types regenerated
- [x] 6.2 `docs/domain-map.md` attention section: "need", "Later", what items hold
- [x] 6.3 CHANGELOG updated
- [ ] 6.4 Gates green
- [ ] 6.5 UI verified at 375px and desktop
- [ ] 6.6 Archive the change
