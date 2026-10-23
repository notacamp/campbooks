## ADDED Requirements

### Requirement: IDLE settings never stop a server that does not run IDLE
The server SHALL refuse to start on an IDLE safety poll
(`CAMPBOOKS_CONNECTORS__IMAP__IDLE_POLL_SECONDS`) of zero. It SHALL tie that
poll to the instance's poll bounds (`CAMPBOOKS_CONNECTORS__POLL__MIN_SECONDS`,
`…__MAX_SECONDS`) only when IMAP and IDLE are both enabled. A server with IDLE
off SHALL start whatever its poll bounds.

#### Scenario: Narrowed poll bounds with IDLE off
- **WHEN** an instance sets its poll bounds to 60–600 seconds and leaves IDLE off
- **THEN** the server starts

#### Scenario: Narrowed poll bounds with IDLE on
- **WHEN** the same instance turns IMAP and IDLE on with the default safety poll of 900 seconds
- **THEN** the server refuses to start, naming `CAMPBOOKS_CONNECTORS__IMAP__IDLE_POLL_SECONDS`
