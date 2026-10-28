# Tasks

## 1. Conversations: people copied on received mail

- [x] 1.1 `flagged_sender_sql!` takes the address column (`()` keeps `k.address`)
- [x] 1.2 `list_correspondents`: To and Cc of received mail as copies, own addresses left out, ranked after `received` by copies, `received` counting only mail from them
- [x] 1.3 Safety for the only copied: not no-reply, not flagged, newest copying message's sender may be written to and is not flagged
- [x] 1.4 HTTP tests: copied in Cc and in To, ranking after received and by copies, copied by a sender under review / reported / failing its checks left out unless written to, copied no-reply left out, never the inbox's own address

## 2. Close

- [x] 2.1 OpenAPI and client types (the ranking description)
- [x] 2.2 Changelog entry file in `changelog/unreleased/`
- [x] 2.3 Gates green (CI)
- [x] 2.4 Recording: the tests and a copied person suggested
- [x] 2.5 `openspec archive suggest-copied-correspondents --yes`
