# Tasks

## 1. Schema and settings
- [ ] 1.1 Migration `20261023150000_connectors__imap_locations.sql` with RLS and the `(stream_id, message_key)` index
- [ ] 1.2 `ImapConfig` gains `backfill_days`, `max_message_bytes`, `run_budget_bytes`, `sweep_minutes`, `sessions_per_host`, `max_folders`

## 2. Adapter
- [ ] 2.1 `imap/folders.rs`: LIST (+ `RETURN (SPECIAL-USE)` when offered), modified UTF-7 decode, roles from attributes then the name table, exclusions, `MAX_FOLDERS`
- [ ] 2.2 `imap/state.rs`: cursor (de)serialisation; location union → `in_inbox`/`in_spam`/`in_trash`/`read`/`starred`/direction/labels
- [ ] 2.3 `imap/read.rs`: STATUS pass, new-UID fetch, CONDSTORE/QRESYNC flag pass, backfill walk, sweep, UIDVALIDITY re-walk, Message-ID identity (digest fallback, OBJECTID tie-break), `BODY.PEEK[]` only, byte/message budgets, oversized
- [ ] 2.4 `sync_stream`'s `imap` arm returns the `RunPage`; locations written in the same transaction as the index
- [ ] 2.5 Session cap per host through `provider_budgets`; health: 3 consecutive login refusals → `needs_reauth`; `UNAVAILABLE`/`INUSE`/`LIMIT`/refused → throttle

## 3. Tests
- [ ] 3.1 Extend `testkit_imap.rs`: a stateful fake server (folders, UIDs, flags, MODSEQ, UIDVALIDITY reset, EXPUNGE, MOVE in "another app", servers with and without SPECIAL-USE / CONDSTORE / QRESYNC)
- [ ] 3.2 One test per scenario in the spec delta, plus: Message-ID missing, the same message in two folders, a folder renamed/removed, a run stopping at its byte budget and continuing

## 4. Finish
- [ ] 4.1 `cargo xtask openapi` (events unchanged; regenerate to prove it)
- [ ] 4.2 Changelog entry `changelog/unreleased/added/311-imap-reading.md` (still off by default)
- [ ] 4.3 Gates green in CI; vhs recording of a backfill + an incremental run against the fake server
