## 1. Conversations

- [x] 1.1 `Flag::{Trash, Untrash, Spam, Unspam}` and `RequestedChange` kinds
- [x] 1.2 Operations `trash` / `untrash`, `report_spam` / `not_spam` with undo pairs; tests (lists, provider items, readers, strangers)

## 2. Connectors

- [x] 2.1 Contract `changes_requested` v1: the four kinds
- [x] 2.2 Migration widening `outbound_changes.kind`
- [x] 2.3 Gmail trash/untrash and spam, Zoho moves and `moveToSpam`, each remembering where the message was; unit tests and Gmail and Zoho flow tests (incl. a missing Trash folder)

## 3. Finish

- [x] 3.1 OpenAPI and events catalogue regenerated, client types regenerated
- [x] 3.2 CHANGELOG
- [x] 3.3 Gates green in CI
