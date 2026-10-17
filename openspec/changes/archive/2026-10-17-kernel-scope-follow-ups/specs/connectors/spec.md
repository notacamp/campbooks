## MODIFIED Requirements

### Requirement: The dispatcher only reads and leases the schedule
The dispatcher SHALL work in the maintenance scope (`begin_maintenance`,
allowed for it under `[[access.maintenance_scope]]` in `architecture.toml`),
in which it SHALL be able to read the scheduling index and lease due
streams, and nothing else: no kernel table admits that scope, so it SHALL
enqueue each stream's run in the stream's workspace, set in the same
transaction as the lease. It SHALL NOT be able to insert or delete schedule
rows, and a transaction that set neither a workspace nor the maintenance
scope SHALL see no schedule row; the kernel scope SHALL NOT admit it to the
schedule either.

#### Scenario: The policies
- **WHEN** the database's policies are listed
- **THEN** no module policy admits the kernel scope, the only module policies admitting the maintenance scope are the schedule's `FOR SELECT` and `FOR UPDATE` ones and the needs backfill's, and none admits a transaction merely for setting no workspace

#### Scenario: Nothing set
- **WHEN** a transaction of the runtime role sets no workspace and no scope
- **THEN** it sees no row of the scheduling index

#### Scenario: Due streams of two workspaces
- **WHEN** streams of two workspaces are due and the dispatcher ticks, as the runtime role
- **THEN** each stream's row is leased and its run is enqueued in its own workspace
