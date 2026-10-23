# Tasks

## 1. Conversations: ask texts

- [ ] 1.1 `put_texts` writes an optional `asks: [{key, title, amount?}]` section beside `items` in both layers, and the blob is written when any item or ask has text
- [ ] 1.2 `contracts/read-texts.v1.schema.json` gains the optional `asks` section; tests for a read with asks only

## 2. Tasks: the consumer

- [ ] 2.1 Migration (after `20261031000300`): `tasks.tasks.ask_key text` + unique index `(workspace_id, ask_key)` where not null
- [ ] 2.2 Inbound copy of `message_read` (asks, inbox, owner, texts) and of the texts blob in `tasks/src/inbound.rs`; contract README lists tasks as a consumer of `read-texts`
- [ ] 2.3 `tasks.add_read_asks` handler: skip with no owner; per ask, skip a known `ask_key`; `same_as` checked (exists, open, owner's, same conversation or sender inbox) → fill an empty due only; structured-then-model `pay` guard; otherwise create origin `ask`, actor `scout`, title from texts (fallback by verb), amount, source the conversation, container the inbox, `ask_key`; history and `task_changed` in the same transaction
- [ ] 2.4 Server tests: create, repeated delivery, `same_as` (valid, foreign, closed), due filled only when empty, shared inbox skipped, no texts, two layers of one invoice, workspace isolation

## 3. Attention

- [ ] 3.1 The list hides a conversation's `do`/`pay` item when the caller's task item from that conversation is open or was cleared at or after the item's `since`; tests

## 4. Client

- [ ] 4.1 Task detail for origin `ask`: "Scout found this in the email" above the source block, in four languages; test; Storybook screen story

## 5. Close

- [ ] 5.1 OpenAPI and client types regenerated if anything changed
- [ ] 5.2 Changelog entry file
- [ ] 5.3 Gates green
- [ ] 5.4 Recording (375px + desktop) of Home and the task detail with a task from an email
- [ ] 5.5 `openspec archive tasks-from-email-asks --yes` before merge
