## 1. At most once

- [x] 1.1 Never take over a pending key; abandoned pending keys answer 409 `idempotency_outcome_unknown`
- [x] 1.2 The message in en, pt-PT, es-ES and fr

## 2. Tests

- [x] 2.1 A crash between the commit and the stored answer (simulated) never runs twice
- [x] 2.2 Two requests with one key at once run once; the retry replays
