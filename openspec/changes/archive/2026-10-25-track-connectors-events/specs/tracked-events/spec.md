## ADDED Requirements

### Requirement: The connectors module's changes are tracked
Every change the connectors module makes on someone's behalf SHALL be
tracked as an event of the tracking plan, in the transaction that makes
it, with a snapshot read from the module's own rows in that transaction: a
connection added, reconnected or removed (with why, and what becomes of
its mail), a connection's health changing (with the health before), a
member asking for a connection to sync now, a sync run that stored
changes or moved its stream's phase (one summary per run, with counts,
never one event per message), the outbound changes a run made at the
provider (one summary per run, with counts by how they ended), a calendar
change ending, and a send succeeding or failing (with the kind of
failure). A change that changes nothing SHALL track nothing. Events SHALL
carry ids, enums, booleans, counts and times only: never an address, a
display name, a host, a username, a folder or label name, a subject, a
provider's id or a provider's error text. Every mutating operation, job,
schedule and handler of the module SHALL have a Coverage row.

#### Scenario: Disconnecting a mailbox
- **WHEN** the member who connected a mailbox disconnects it
- **THEN** one `connectors.connection_removed` event is written with actor `user`, operation `connectors.disconnect`, the connection as it stood, cause `disconnected` and the mail choice, and no address

#### Scenario: A sync run
- **WHEN** a stream's run stores a batch of new mail
- **THEN** one `connectors.stream_synced` event is written as `system` with the stream afterwards and the number of changes and batches, and a later run that finds nothing tracks nothing

#### Scenario: A grant stops working
- **WHEN** a run finds the grant revoked at the provider
- **THEN** one `connectors.connection_health_changed` event is written with `before` `ok` and the connection's health `needs_reauth`, reason `grant_revoked`

#### Scenario: A refused send
- **WHEN** the provider refuses a message for good
- **THEN** one `connectors.send_failed` event is written with the send, state `failed`, and error `rejected`
