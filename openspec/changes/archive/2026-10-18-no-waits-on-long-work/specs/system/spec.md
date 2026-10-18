## MODIFIED Requirements

### Requirement: Bounded database waits
Every connection of the runtime pool SHALL start with a `statement_timeout`
of 30 seconds and a `lock_timeout` of 10 seconds, so no request waits on the
database without end. A job's own transaction SHALL allow its statements 5
minutes. Migrations SHALL run on their own connection, without these
limits. A statement cancelled at its timeout, a lock not obtained in time,
or no connection free within the pool's acquire timeout SHALL answer 503
`database_unavailable`, logged by kind with no data; a job meeting one SHALL
be retried. Work over many rows (replaying a type's logged events, pruning
expired sign-in state) SHALL run in batches of at most 5,000 rows a
statement, so none nears these limits.

#### Scenario: A lock held elsewhere
- **WHEN** a request's statement waits on a lock another transaction holds for longer than 10 seconds
- **THEN** the statement is cancelled and the request answers 503 `database_unavailable` instead of hanging

#### Scenario: A job's longer statement
- **WHEN** a job's own transaction runs a statement that takes a minute
- **THEN** it completes, while the same statement in a request would be cancelled after 30 seconds

#### Scenario: Replaying many events
- **WHEN** an operator replays a consumer's events and tens of thousands are logged
- **THEN** they are enqueued 5,000 at a time, each batch in its own transaction, and each event once
