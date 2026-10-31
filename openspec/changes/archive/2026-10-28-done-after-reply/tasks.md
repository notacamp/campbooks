## 1. Conversations
- [x] 1.1 `conversations.member_replied` v1 event, contract and catalogue entry
- [x] 1.2 Publish it from `apply_changes` for a new outbound message of a personal inbox; tests (once, shared inbox none, contract)

## 2. Tasks
- [x] 2.1 Migration: `replied_at`, `replied_message`, `reply_answered_at`, history values
- [x] 2.2 `tasks.notice_replies` handler; tests (Do marked, Pay not, other conversation, other member, closed, older reply, repeat, newest kept)
- [x] 2.3 `TaskView.replied` and `asks_done`; `task_changed` gains `asks_done` and `replied_at`
- [x] 2.4 `tasks.answer_reply` and `tasks.unanswer_reply`; tests (flips, undo, newer reply asks again, revision 409, closed, not yours)
- [x] 2.5 Tracking patch 0020 and tracked events
- [x] 2.6 Scout's handbook

## 3. Attention
- [x] 3.1 Migration `items.asks_done`; project it; a task that asks is for today; test

## 4. Client
- [x] 4.1 Home row (chip, line, Done, Not yet, thread, Ask Scout, why Scout asks), `item.open` shortcut
- [x] 4.2 Tasks row chip and detail question, "Your reply" in the source
- [x] 4.3 The thread note's question
- [x] 4.4 Strings in en, pt-PT, es-ES and fr; stories for the new states

## 5. Ship
- [x] 5.1 OpenAPI regenerated, events catalogue, client types regenerated
- [x] 5.2 Tracking plan regenerated
- [x] 5.3 Changelog entry file
- [x] 5.4 Gates green in CI
- [x] 5.5 UI verified at 375px and desktop (stories and the copy-fit test; recording in the PR)
