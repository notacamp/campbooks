## MODIFIED Requirements

### Requirement: Liveness and connection lifetime
The stream SHALL send a heartbeat at a configured interval while idle, and
SHALL end cleanly with a `closing` frame after a configured maximum
connection time, never later than the caller's access token expires, so the
client reconnects with its cursor and is authorized again. Whenever the
stream resolves the caller's membership again (at least once a heartbeat
interval, busy or idle), it SHALL also ask whether the caller's token still
stands (the API's token-revocation check); when it no longer does (the
account was suspended or signed out everywhere), the stream SHALL send
`revoked` and end, and reconnecting with that token SHALL answer 401.

#### Scenario: Idle stream
- **WHEN** no change is recorded for longer than the heartbeat interval
- **THEN** the client receives a heartbeat

#### Scenario: Token about to expire
- **WHEN** the caller's access token expires before the maximum connection time
- **THEN** the stream sends `closing` and ends no later than the expiry

#### Scenario: The caller's tokens are revoked
- **WHEN** the account of a person following a workspace's changes is suspended or signs out everywhere while the stream is open
- **THEN** within one heartbeat interval the stream sends `revoked` and ends, and opening it again with the same token answers 401
