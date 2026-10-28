## ADDED Requirements

### Requirement: Listing shared Notion databases
`connectors.list_notion_data_sources` SHALL answer the Notion databases the
caller's own Notion connection can see (id, title, entry count when known),
100 at a time with a cursor, within the connection's request budget. It SHALL
answer 404 for a connection that is not the caller's or when Notion databases
are off.

#### Scenario: Another member's connection
- **WHEN** Rui asks for the databases of Marta's Notion connection
- **THEN** the response is 404 and Notion is not called
