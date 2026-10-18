## ADDED Requirements

### Requirement: Token refresh never waits on a lock
An expired access token SHALL be refreshed once per connection however many
runs need it: the run that takes the connection's refresh lock (only ever
tried, never waited on) SHALL refresh and store it, and the others SHALL use
the stored token once it is there, looking again every 100 ms for at most
the provider call's limit and five seconds more, or refresh themselves if
the lock is freed without a token. Waiting for another run's refresh SHALL
never fail a run or use up one of its attempts.

#### Scenario: A send during a slow refresh
- **WHEN** a sync is refreshing a connection's token and Google takes 1.5 seconds to answer, and a send of the same connection runs meanwhile
- **THEN** the send uses the refreshed token and is sent on its first attempt, and Google is asked for a token once
