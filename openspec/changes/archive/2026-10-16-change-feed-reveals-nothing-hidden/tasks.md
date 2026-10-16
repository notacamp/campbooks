# Tasks

## 1. Kernel

- [x] 1.1 `crypto::Signer::seal`/`open` (AES-256-GCM under a key derived from the instance secret, purpose `campbooks kernel sealed values v1`); `Cipher::seal_bytes`/`open_bytes`
- [x] 1.2 Migration `kernel.changes.sequenced_at` (backfilled from `occurred_at`, checked against `seq`), stamped by the sequencer with `statement_timestamp()`; pruning by it
- [x] 1.3 Stream: sealed `Position { seq, at }` tokens on `ready`/`change`/`heartbeat`/`reset`; no `head`; resume rule by token age only, completeness kept as a safety net; `too_far_behind` and `max_backlog` removed
- [x] 1.4 Heartbeats on a fixed timer from the open, busy or idle; the heartbeat token moves past what was read only after a read that started after the heartbeat's time reached the end of the feed
- [x] 1.5 No per-page collapsing of a record's changes
- [x] 1.6 Unit tests: the start rule (age alone, unknown, missing changes), tokens (fresh, one length, workspace-bound, legacy numbers refused), sealing

## 2. Server

- [x] 2.1 The stream's OpenAPI description; `openapi.json` regenerated
- [x] 2.2 Tests updated for tokens; a long hidden backlog never resets; unknown, altered, foreign and expired tokens reset
- [x] 2.3 The matrix: member, manager, a client acting for a member and a client acting for nobody, each in a busy and a quiet workspace (mail in a hidden inbox, someone else's draft, someone else's connection, a client's draft); frames, tokens, heartbeat cadence and resumes compared

## 3. Client

- [x] 3.1 `ready` without `head`; generated API types regenerated
