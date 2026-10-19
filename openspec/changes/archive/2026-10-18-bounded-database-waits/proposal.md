## Why

A change-feed test stalled for about 20 minutes after auth
(campbooks-cloud#249, #251). The runtime pool bounded only how long a
request waits for a connection (`acquire_timeout`), never how long a
statement runs or waits for a lock. A lock wait, for example behind the
sequencer's `FOR UPDATE` on `kernel.change_heads`, or a transaction left
open elsewhere, could hold a request indefinitely, in production as well as
in tests.

## What Changes

- Every runtime connection starts with `statement_timeout` = 30 s and
  `lock_timeout` = 10 s. They are set through the connection's startup
  `options` (`db::with_session_limits`), so they hold before the first query
  and whatever role the connection switches to. The tests' runtime-role pool
  gets them too.
- A job's own transaction (`JobContext::tx`) allows its statements 5 minutes
  (`SET LOCAL`, until it ends): jobs work in larger batches, still bounded.
  Migrations connect on their own, without these limits, so
  `CREATE INDEX CONCURRENTLY` and long migrations are unaffected.
- A statement cancelled at its timeout (SQLSTATE 57014), a lock not had in
  time (55P03), or no connection free within the acquire timeout, answers
  503 `database_unavailable` with a warning logged by kind. Before, it was a
  500 `internal_error`. A job meeting one is retried, as before.
- Checked: the change feed's open and reads each hold a short transaction
  (one read, then commit), and none waits on anything but the database
  while one is open.

## Impact

- Specs: system (Bounded database waits).
- Code: `kernel::db` (`with_session_limits`, `allow_statements_for`, the
  limits), `kernel::error` (`gave_up_waiting`), `jobs::context`,
  `testing::app_role_pool`.
- No migration, no new setting. Nothing changes on screen, except that a
  request stuck on the database now ends with "The database is unavailable
  right now" instead of hanging.
