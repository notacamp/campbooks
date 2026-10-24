# Tasks

## 1. Data and settings
- [x] 1.1 Migration `20261102010000_connectors__imap_send.sql`: `connectors.sends` gains `sent_copy`, `sent_copy_raw`, `sent_copy_reason`, `sent_copy_attempts` and the partial index (row-level security unchanged)
- [x] 1.2 `CAMPBOOKS_CONNECTORS__IMAP__SEND_*` in `config.rs` (checked only while IMAP is on) and `deploy/.env.example`
- [x] 1.3 `imap_servers::smtp_server`; the SMTP hosts known to keep a copy (`imap::SENT_KEEPERS`)

## 2. The SMTP submission
- [x] 2.1 `imap/smtp.rs`: a submission over a connection Campbooks opens to the guard's address (TLS or STARTTLS required), bounded reads, a time limit per step, `AUTH`, EHLO `SIZE`, `MAIL`/`RCPT`/`DATA` then the message as separate steps, failures classified (before `DATA`, definite answer, unknown)
- [x] 2.2 `send_message/via_imap.rs`: `From` check, envelope (To, Cc, Bcc), wire message without `Bcc`, size, budgets and the session slot, the attempt committed after 354, definite and unknown outcomes, refused password counted, the sent message indexed and the copy `pending` (or `provider`)
- [x] 2.3 `send_message`: the IMAP arm configured when IMAP is on, its quiet window, its cursor

## 3. The Sent copy
- [x] 3.1 `syntax`: `APPENDUID`; `session`: `APPEND` as a synchronising literal through the bounded framer
- [x] 3.2 `imap/sent.rs`: the Sent folder, `APPENDLIMIT` and the size cap, the search for the `Message-ID`, the `APPEND`, `UNSELECT`, the ends
- [x] 3.3 `sync_stream`: pending copies taken after the write-backs, recorded (ends, blob released, locations) before the read; busy and cut sessions
- [x] 3.4 The reader keeps sent messages whose copy could not be made

## 4. Tests
- [x] 4.1 The SMTP fake takes mail (`MAIL`, `RCPT`, `DATA`, dot-unstuffing), announces `SIZE`, is busy once, drops the connection after the message, floods EHLO
- [x] 4.2 The IMAP fake takes `APPEND` (`APPENDUID` with UIDPLUS, `APPENDLIMIT`), busy, refusing and hanging variants
- [x] 4.3 One test per scenario of the spec delta
- [x] 4.4 Settings boot with IMAP off and with production's poll bounds

## 5. Finish
- [x] 5.1 OpenAPI, events and tracking plan unchanged (CI's drift checks); no Coverage row added
- [x] 5.2 Changelog entry `changelog/unreleased/added/313-imap-send.md` (still off by default)
- [ ] 5.3 Gates green in CI; recording
