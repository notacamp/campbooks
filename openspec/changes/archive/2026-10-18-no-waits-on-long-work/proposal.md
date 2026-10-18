## Why

#577 (campbooks-cloud#251) bounded every runtime statement at 30 s and every
lock wait at 10 s. Its review found work that those limits now cut short
(campbooks-cloud#254):

- **Token refresh:** it held `pg_advisory_xact_lock` while calling the
  provider, which can take up to 60 s. A concurrent sync or send waited on
  that lock, met the 10 s `lock_timeout` (55P03), and used up a job attempt.
  Sends have few attempts.
- **`events replay`:** one `INSERT … SELECT` over every logged event of a
  type, on the runtime pool, now capped at 30 s.
- **Identity prune:** each expired-state `DELETE` was one unbatched
  statement.

## What Changes

- The per-connection refresh lock is only ever tried
  (`pg_try_advisory_xact_lock`), never waited on. The run that gets it
  refreshes and stores the token, as before. A run that doesn't looks for
  that token every 100 ms until it is stored (at most the provider call's
  60 s plus 5 s), then uses it. If the lock is freed first because the
  refresh failed, it refreshes itself. A send or sync that arrives during
  a slow refresh therefore succeeds on the same attempt, with one call to
  the provider.
- `events replay` reads 5,000 events per batch, each batch in its own
  transaction, continuing after the last event read. Events that already
  have an unfinished delivery are still skipped.
- The identity prune deletes at most 5,000 rows per statement, repeating
  until less than a batch is left.
- A test shows that a job's own transaction (`JobContext::tx`) really
  allows its statements 5 minutes.

## Impact

- Specs: connectors (Token refresh never waits on a lock), system (Bounded
  database waits: long work in batches).
- Code: `connectors::domain::credentials`, `kernel::events` (replay),
  `identity::prune`, the fake Google's slow refresh; tests.
- No migration. Nothing changes on screen.
