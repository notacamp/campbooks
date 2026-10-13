# Tasks

## 1. Design

- [x] 1.1 Proposal, design (alternatives: where write-backs run, per-message vs per-thread vs batch, echo suppression, conflict rule), spec delta

## 2. Connectors

- [x] 2.1 Migration `connectors__outbound_changes` (table, indexes, RLS, grant) and `gmail_messages.reemit`
- [x] 2.2 Gmail client: `messages.modify`, `messages.batchModify`; history records carry their id and label ids
- [x] 2.3 `connectors.request_changes` consumer: map intents to labels, stamp with `getProfile`, record, run now
- [x] 2.4 Push step in `connectors.sync_stream`: merge, later-writer check over history, write (single or batch, batch fallback), trouble path, outcome and index update, prune and expiry
- [x] 2.5 Reader: re-emit flagged messages, pending overlay; continuation when rows arrive during a run

## 3. Tests (fake Google, in-process)

- [x] 3.1 Fake Google: modify, batchModify, history records with label ids, refusing message, 5xx
- [x] 3.2 Every intent kind; skipped labels and Receipts; contract of the consumed event
- [x] 3.3 Echo suppression: no batch after a write-back, a Gmail change still arrives, pending overlay
- [x] 3.4 Conflicts: Gmail later wins, Gmail earlier loses, undo during a write
- [x] 3.5 Retries (429 then success, 5xx backoff), permanent failure, batching

## 4. Finish

- [x] 4.1 `.sqlx` regenerated, OpenAPI and events catalogue unchanged or regenerated
- [x] 4.2 CHANGELOG, decisions-needed entry for labels made in Campbooks
- [x] 4.3 Gates green; terminal recording against the server with the fake Google
