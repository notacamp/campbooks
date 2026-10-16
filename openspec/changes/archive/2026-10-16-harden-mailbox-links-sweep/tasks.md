## 1. Kernel and connectors
- [x] 1.1 `GrantIntake::connection_of` (and `NoGrantIntake`)
- [x] 1.2 The connectors module answers it from its own rows, for the grantor only; test
## 2. Identity
- [x] 2.1 The sweep asks the port first, names what is connected, settles one account per transaction in `account_id` order
- [x] 2.2 `record` writes an unnamed row afresh on a retried connect; test
- [x] 2.3 A panic in the hand-over redirects with `link_error=internal_error`; test
- [x] 2.4 Tests: a connected mailbox left unnamed keeps its way in; the fake intake tracks connections
## 3. Docs
- [x] 3.1 `CHANGELOG.md`
