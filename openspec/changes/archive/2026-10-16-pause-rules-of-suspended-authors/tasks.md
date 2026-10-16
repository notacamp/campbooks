# Tasks

## 1. identity

- [x] 1.1 `identity.account_suspended` carries `at` (read under the account lock)
- [x] 1.2 `identity.account_unsuspended`, published when a suspension is lifted; tests

## 2. workspaces

- [x] 2.1 Read the identity events; `workspaces.member_suspended` / `member_unsuspended`
- [x] 2.2 Fan-out handlers: every live workspace of the account, one transaction each; tests (two workspaces, another user, no user)

## 3. conversations

- [x] 3.1 Migration `conversations__member_suspensions` with forced RLS; deleted with the workspace
- [x] 3.2 Handlers: newest `at` wins, duplicates and late deliveries change nothing, `rule_changed` announced
- [x] 3.3 `author_suspended` in the rule queries and `effective_state`; `RuleView.paused_reason`
- [x] 3.4 Tests: paused and resumed through the events, out of order, apply run cancelled at its next batch, new mail untouched

## 4. Client

- [x] 4.1 The rules list's paused line names the reason, in four languages; test

## 5. Ship

- [x] 5.1 `openapi.json`, `events.json`, client types regenerated
- [x] 5.2 CHANGELOG, recording, archive the change
