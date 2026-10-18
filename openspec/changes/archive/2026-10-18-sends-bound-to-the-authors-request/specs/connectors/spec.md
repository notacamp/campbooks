## ADDED Requirements

### Requirement: Outbound changes are bound to the request they were recorded for
When the connector records an outbound change (after the outbound-requests
port confirmed its request), it SHALL store, for each row it inserted, a MAC
under the binding key derived from the instance secret, over the
workspace, the row, its stream, its request, the provider message id, the
kind and the labels to add and remove. Before a stream's run makes any
change at the provider it SHALL verify each pending change's MAC; a change
whose MAC does not verify, or that has none, SHALL NOT be made: it SHALL end
`skipped` with the reason `unverified`, its message SHALL be re-read from
the provider so Campbooks shows the provider's state, and a WARN line
carrying ids and a count only SHALL be logged. Without an instance secret
no change SHALL be made at the provider.

#### Scenario: A change planted with SQL
- **WHEN** a transaction of the runtime role in a workspace inserts a pending outbound change no member asked for
- **THEN** the next run writes nothing to the provider and the change ends `skipped` (`unverified`)

#### Scenario: A member's change edited with SQL
- **WHEN** a member's recorded star is turned into an archive before the run
- **THEN** the run writes nothing to the provider for it and it ends `skipped` (`unverified`)

#### Scenario: A member's change as recorded
- **WHEN** a member stars a message and the connector records the change
- **THEN** the run makes it at the provider
