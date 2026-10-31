## MODIFIED Requirements

### Requirement: Adaptive, fair sync
Each connection and capability SHALL be read as a stream with a cursor. A
stream SHALL be synced by at most one job at a time; requests to sync a
stream that already has a pending run SHALL collapse into it. A run SHALL
read one bounded page and SHALL be scheduled again at once when more remains,
at the minimum interval after a page with changes, and at a doubling
interval up to the maximum while nothing changes. A stream's next poll
SHALL be made due a short slack (15 seconds) before its interval ends, so
the dispatcher's tick that the interval reaches takes it, and SHALL never be
made due sooner than a wait that must be kept whole (a request budget's
wait, an IDLE or notification safety poll). A Zoho mailbox's interval SHALL
never exceed `CAMPBOOKS_CONNECTORS__ZOHO__POLL_MAX_SECONDS` (default 60, 30
to 14400), whatever the global maximum; a missing value SHALL use the
default, and an unusable one SHALL use the default with a warning at startup
and SHALL never stop the server. `connectors.sync_now` SHALL make the stream
due now at the minimum interval. Requests to a provider SHALL draw from a
per-provider budget shared by every process.

#### Scenario: Idle mailbox backs off
- **WHEN** a stream's incremental runs find no change several times in a row
- **THEN** its interval doubles each time and stops at the configured maximum

#### Scenario: Sync now collapses
- **WHEN** a member calls sync now twice while the stream's run is pending
- **THEN** one run is pending and the stream's interval is the minimum

#### Scenario: Large backfill does not starve others
- **WHEN** one workspace backfills a large mailbox and another workspace's stream becomes due
- **THEN** the second stream's run is claimed before the first workspace's next page

#### Scenario: A quiet stream runs on the tick its interval reaches
- **WHEN** a quiet stream with a 120 second interval finishes a run one second after a dispatcher tick
- **THEN** it is made due 105 seconds later and the tick two minutes after the first takes it, not the tick three minutes after

#### Scenario: A wait is kept whole
- **WHEN** a run's least wait before its next poll (a budget's wait, or an IDLE or notification safety poll) is longer than its interval less the slack
- **THEN** the stream is made due after exactly that wait

#### Scenario: A quiet Zoho mailbox is read every minute
- **WHEN** a Zoho mail stream finds no change several times in a row, with the global maximum at 120 seconds or more
- **THEN** its stored interval stays at 60 seconds and its next poll is due 45 seconds after the run

#### Scenario: An unusable Zoho poll maximum does not stop the server
- **WHEN** the server starts with `CAMPBOOKS_CONNECTORS__POLL__MIN_SECONDS=60`, `CAMPBOOKS_CONNECTORS__POLL__MAX_SECONDS=120` and `CAMPBOOKS_CONNECTORS__ZOHO__POLL_MAX_SECONDS` set to `abc`, `0` or `99999`
- **THEN** it starts, logs a warning naming the variable, and polls Zoho mail at most 60 seconds apart

