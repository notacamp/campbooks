## Why
#501 (harden-security-alert-auth) merged with one known gap: the
`conversations.refresh_need` follow-up job works a conversation's need out
from reads made before it waits for its locks. Under READ COMMITTED the
writer it waited for may have archived the conversation, or another refresh
may have stored the first need meanwhile, so the job could store an open
need on an archived conversation or overwrite a member's Done with a stale
open need. Found in the final security review of #501.

## What Changes
- Once the job holds both locks, it works the refresh out again, once, in
  the same transaction, so every read is fresh.
- The job's previous `lock_timeout` is restored with a transaction-local
  `set_config`, not a session-wide `RESET`.
- The design's lock-order notes are corrected: the job's wait is bounded
  (deadlock detector or lock timeout), not cycle-free; rules lock the
  conversation, then messages, then the check.

## Impact
- `crates/modules/conversations/src/domain/needs.rs`, `features/needs/tests.rs`.
- Spec: `conversations` → "Spam and trash need nothing" (two scenarios).
- No migration, no API change.
