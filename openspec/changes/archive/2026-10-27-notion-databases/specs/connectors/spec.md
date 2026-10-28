## ADDED Requirements

### Requirement: Notion databases reach Campbooks
With Notion databases enabled, each linked Notion database SHALL have a
change stream. It reads the schema when it changed, and reads the entries
edited since its cursor. It SHALL emit normalised, hashed values, and SHALL
stay within the connection's request budget. The webhook events
`data_source.content_updated` and `data_source.schema_updated` SHALL only
mark that database's streams due.

#### Scenario: A schema change in Notion
- **WHEN** Rui adds a column "Region" to the Notion database "Clients"
- **THEN** the next run reads the new schema and emits it, and nothing is written to Notion

### Requirement: Campbooks changes reach Notion databases
Schema changes, new rows, cell updates and row trashing SHALL be made in
Notion only from MAC-bound outbound changes verified before the call. A
change whose base edit time or schema hash moved SHALL be held back. Removing
a Notion column SHALL be made only from a change that carries the member's
confirmation. A Notion database SHALL never be deleted.

#### Scenario: A column removal without confirmation
- **WHEN** an outbound change to remove a Notion column lacks the confirmation
- **THEN** it is not sent, and nothing changes in Notion
