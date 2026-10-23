# Tasks

## 1. The IMAP client
- [ ] 1.1 `syntax`: response codes keep their UID sets (`COPYUID`, `MODIFIED`), bounded
- [ ] 1.2 `folders`: the `stream` role (a folder named like a stream mark), modified UTF-7 encoding, the checked builder of a folder name Campbooks creates
- [ ] 1.3 `state`: stream folders count as the inbox and are not labels; `provider_stream` when the server shows where a message is; the cursor keeps the folders Campbooks created

## 2. Recording
- [ ] 2.1 `outbound::map_imap`: flags, inbox, trash, spam and stream marks; labels `unsupported`
- [ ] 2.2 `request_changes`: the IMAP arm maps and records changes as pending

## 3. Writing
- [ ] 3.1 `imap/write.rs`: the plan over the shared merge (the index's tokens seen as flags, inbox, bins and marks), the moves per intent, the flags per copy
- [ ] 3.2 Per folder: `SELECT` (CONDSTORE), UIDVALIDITY check, `UID SEARCH` for what is gone, conditional `STORE`, `UID MOVE` or `UID COPY` + `\Deleted` + `UID EXPUNGE` (UIDPLUS), `UNSELECT`; never `EXPUNGE` or `CLOSE`
- [ ] 3.3 Stream folders: delimiter (`LIST "" ""`), name, `CREATE` + `SUBSCRIBE`, cap, the cursor
- [ ] 3.4 The outcome: ends, locations (flags, `COPYUID`), index tokens, cursor (`recheck`)
- [ ] 3.5 `sync_stream`: expire and take the backlog, push before the read over the same session, record the push before reading, busy and wire trouble
- [ ] 3.6 The reader honours `reemit` and shows pending changes as made
- [ ] 3.7 `CAMPBOOKS_CONNECTORS__IMAP__MAX_CREATED_FOLDERS` in `config.rs` and `deploy/.env.example`

## 4. Tests
- [ ] 4.1 The fake server: `SELECT`, `STORE` (CONDSTORE `UNCHANGEDSINCE`), `MOVE`, `COPY`, `UID EXPUNGE`, `EXPUNGE`, `CLOSE`, `CREATE`, `SUBSCRIBE`, `UNSELECT`, `COPYUID`; servers with and without MOVE, UIDPLUS and CONDSTORE; one that accepts without applying, one busy on writes; every command it received
- [ ] 4.2 One test per scenario of the spec delta, plus the folder-name builder, modified UTF-7 and the mapping

## 5. Finish
- [ ] 5.1 OpenAPI, events and tracking plan unchanged (CI's drift checks); no Coverage row added
- [ ] 5.2 Changelog entry `changelog/unreleased/added/312-imap-write-back.md` (still off by default)
- [ ] 5.3 Gates green in CI; recording
