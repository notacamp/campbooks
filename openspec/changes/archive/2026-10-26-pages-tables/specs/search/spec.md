## ADDED Requirements

### Requirement: Table rows are searchable
With tables enabled, search SHALL index each row's title and text values
through the pages `SearchSource`, answering a row only to a caller who may
read it.

#### Scenario: A row in a private table
- **WHEN** Rui searches for a client name that is only in Marta's private table
- **THEN** the row is not among Rui's results
