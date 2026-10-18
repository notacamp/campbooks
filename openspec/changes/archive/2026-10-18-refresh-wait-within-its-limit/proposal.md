## Why

The review of #578 (campbooks-cloud#254) found gaps in the tests and the
spec (campbooks-cloud#256):

- The token-refresh test's 1.5 s slow refresh was shorter than the 10 s
  `lock_timeout`, so the old blocking lock would also have passed it.
- The spec said waiting for another run's refresh "SHALL never fail a run",
  but past the provider call's limit (plus five seconds) it does.
- The job-limit test's final check could read another pooled connection.

## What Changes

- The test runs the send on connections whose `lock_timeout` is 200 ms
  (checked), far shorter than the refresh, so code waiting on the lock
  would fail its attempt with 55P03.
- The spec adds "within the provider call's limit" and says what happens
  past it. Its scenario names the 200 ms `lock_timeout`.
- The job-limit test checks every connection of the pool, the job's among
  them, for the 5-minute limit after the job.
- No `kernel.events (name, id)` index yet: replay reads in batches of
  5,000 by id and filters on name, which is enough until one event type is
  large.

## Impact

- Spec: connectors (Token refresh never waits on a lock).
- Tests only: `send_message` tests and `jobs` tests. No behaviour change.
