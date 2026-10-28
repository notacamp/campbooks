## ADDED Requirements

### Requirement: Listing shared Notion databases
`connectors.list_notion_data_sources` SHALL answer the Notion databases the
caller's own Notion connection can see (id, title, how many columns it has,
and whether it is linked to a table already), 100 at a time with a cursor and
at most 200 in all, within the connection's request budget. It SHALL answer
404 for a connection that is not the caller's, and an empty list, without
calling Notion, when Notion databases are off.

#### Scenario: Another member's connection
- **WHEN** Rui asks for the databases of Marta's Notion connection
- **THEN** the response is 404 and Notion is not called

#### Scenario: Notion databases are off
- **WHEN** Marta asks for the databases of her own Notion connection and Notion databases are off on the instance
- **THEN** the answer is an empty list and Notion is not called
