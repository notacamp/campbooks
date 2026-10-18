## 1. Server

- [x] 1.1 `list_senders` `reachable`: leave out senders under an open review in SQL, no-reply addresses and newest mail failing checks or refused by DMARC in Rust; read 40 more rows so the answer rarely shortens
- [x] 1.2 Tests: the phishing sender and no-reply@accounts.example never listed with `reachable`, typed or not; listed without it; the facts unit-tested
- [x] 1.3 OpenAPI and client types

## 2. Client

- [x] 2.1 Forward's picker asks with `reachable`
- [x] 2.2 The draft is recorded on every path; the composer opens only for the current start
- [x] 2.3 Neutral "no file" toast in four locales
- [x] 2.4 Tests: filtered picker, reopen after Esc mid-start, drafts error during a start
- [x] 2.5 Recording scenario `home-ranked-actions` shows the filtered picker
