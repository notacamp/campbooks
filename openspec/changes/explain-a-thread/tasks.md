## 0. Before

- [x] 0.1 Alternatives evaluated (design.md); prototype shown (https://claude.ai/artifact/69QS4V7Cqq6BFA57mAk3q3)
- [x] 0.2 Numbers from the coordinator: migration 20261122010000, tracking patch 0028 (plan version 28)

## 1. Server (conversations)

- [x] 1.1 Migration `20261122010000`: `conversations.explanations`, idempotent, RLS forced, FK cascade
- [x] 1.2 `conversations.explain` (lock, coalesce, 202/200, 409/503, rate limit 20/h) and the job `conversations.explain_thread` (bounded input, strict schema, vetting, interactive cap scope, one retry on empty)
- [x] 1.3 `conversations.get_explanation` with access re-checked on every read (delete on failure), `stale`, `newer` and `refs`; `conversations.forget_explanation`
- [x] 1.4 `ConversationDetail.explain`
- [x] 1.5 Removal: `forget_member`, losing an inbox, the 30-day expiry job
- [x] 1.6 Event `conversations.thread_explained` (ids only) and tracked event (patch 0028)
- [x] 1.7 Tests: access matrix (another member, lost inbox, moved message), coalescing, cap, refusals, vetting, injection-shaped mail, every language

## 2. Scout chat

- [x] 2.1 About-context for `conversations.conversation` carries the member's explanation (3 KiB), with a handbook line

## 3. Client

- [x] 3.1 Button in the note, the `conversations.explain` shortcut with its hint, and the ⌘K command
- [x] 3.2 The card: writing, ready, stale, long thread, limit, failed; chips scroll to and open messages; Hide remembered
- [x] 3.3 "Ask a follow-up" opens the Scout panel about the thread
- [x] 3.4 Phone layout; i18n en, pt-PT, es-ES and fr; stories and tests

## 4. Finish

- [x] 4.1 OpenAPI and client types regenerated; tracking plan applied
- [x] 4.2 Changelog entry `changelog/unreleased/added/407-explain-a-thread.md`
- [ ] 4.3 Gates green in CI
- [ ] 4.4 Recording at 375 px and desktop
- [ ] 4.5 `openspec archive explain-a-thread --yes` before merge
