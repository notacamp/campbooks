## MODIFIED Requirements

### Requirement: The dispatcher only reads and leases the schedule
The dispatcher SHALL work in the kernel scope (`begin_kernel`, allowed for
it in `architecture.toml`), in which it SHALL be able to read the
scheduling index and lease due streams, and nothing else. It SHALL NOT be
able to insert or delete schedule rows, and a transaction that set neither
a workspace nor the kernel scope SHALL see no schedule row.

#### Scenario: The policies
- **WHEN** the database's policies are listed
- **THEN** the only module policies admitting the kernel scope on a module table are the schedule's `FOR SELECT` and `FOR UPDATE` ones and the needs backfill's, and none admits a transaction merely for setting no workspace

#### Scenario: Nothing set
- **WHEN** a transaction of the runtime role sets no workspace and no kernel scope
- **THEN** it sees no row of the scheduling index
