# Tasks

## 1. Kernel

- [x] 1.1 `crypto::Signer::seal`/`open` (AES-256-GCM-SIV under a key derived from the instance secret, purpose `campbooks kernel sealed values v1`); `Cipher::seal_bytes`/`open_bytes`
- [x] 1.2 Migration `kernel.changes.sequenced_at` (backfilled from `occurred_at`, checked against `seq`), stamped by the sequencer with `statement_timestamp()`; pruning by it
- [x] 1.3 Stream: sealed `Position { seq, at }` tokens on `ready`/`change`/`heartbeat`/`reset`; no `head`; resume rule by token age only, completeness kept as a safety net; `too_far_behind` and `max_backlog` removed
- [x] 1.4 Heartbeats on a fixed timer from the open, busy or idle; the heartbeat token moves past what was read only after a read that started after the heartbeat's time reached the end of the feed
- [x] 1.5 No per-page collapsing of a record's changes
- [x] 1.6 Unit tests: the start rule (age alone, unknown, missing changes), tokens (fresh, one length, workspace-bound, legacy numbers refused), sealing
- [x] 1.7 Pages of visible changes: `read_visible` filters by read scopes (`ReadRules::readable`), audience and the caller's containers (`Visibility::container_types` + `visible_containers`, every read) before `LIMIT`; the cursor moves to the read's head; rows checked again in Rust; the heartbeat token takes that head and time
- [x] 1.8 Tokens bound to the caller (associated data); AES-256-GCM-SIV sealing
- [x] 1.9 Opens rate limited per caller (`OPENS_PER_WINDOW` / `OPENS_WINDOW_SECONDS`); startup warning without a secret key; migration notes its lock

## 2. Server

- [x] 2.1 The stream's OpenAPI description; `openapi.json` regenerated
- [x] 2.2 Tests updated for tokens; a long hidden backlog never resets; unknown, altered, foreign and expired tokens reset
- [x] 2.3 The matrix: member, manager, a client acting for a member and a client acting for nobody, each in a busy and a quiet workspace (mail in a hidden inbox, someone else's draft, someone else's connection, a client's draft); frames, tokens, heartbeat cadence and resumes compared
- [x] 2.4 Pages: kernel test of identical pages with and without more hidden changes than a page; server catch-up test; opens rate limit; a token of another caller resets

## 3. Client

- [x] 3.1 `ready` without `head`; generated API types regenerated
