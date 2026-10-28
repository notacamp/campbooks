# Tasks

## 1. Conversations

- [x] 1.1 `done_by` in the ask schema (required enum) and the instruction; `DoneBy` vocabulary
- [x] 1.2 Vet: unknown value is none; `pay` is `elsewhere`
- [x] 1.3 `ReadAsk.done_by` (stored, returned by `get_message_read`), `ReadAskFacts.done_by` (event); contract `conversations.message_read` v1 gains optional `done_by`
- [x] 1.4 Tests: schema, vet, stored, event has the enum and none of the sender's words, a member without access gets 404

## 2. Tasks

- [x] 2.1 Migration: `tasks.tasks.done_by` (nullable, checked)
- [x] 2.2 `add_read_asks` stores it; `notice_replies` skips `elsewhere`
- [x] 2.3 `TaskView.done_by` and `source_ask`
- [x] 2.4 Tests: stored, absent for an older read, skipped / marked, API exposes, events and tracked events carry no sentence
- [x] 2.5 Handbook

## 3. Client

- [x] 3.1 `useAskQuote` in conversations (access-checked), `UseTaskQuote` seam, `TaskStory` quote
- [x] 3.2 en, pt-PT, es-ES, fr quotation marks
- [x] 3.3 Tests and stories (with and without a quote)

## 4. Close

- [x] 4.1 A/B of the instruction against the old one (table in the PR)
- [x] 4.2 Changelog entry file
- [ ] 4.3 Recording (owner session)
- [x] 4.4 `openspec archive done-by-and-quote --yes`
