## ADDED Requirements

### Requirement: Importing a Notion database
With Notion databases enabled, `pages.import_from_notion` SHALL accept a
Notion database the caller's own connection can see, and SHALL make it a
private table of the caller, with a property per column, a row page per
entry, and a default table view, all linked to Notion.

#### Scenario: Importing Clients
- **WHEN** Marta imports the Notion database "Clients" with 38 entries and 7 columns
- **THEN** she has a private table "Clients" with 38 rows and the 7 columns, synced with Notion

### Requirement: Values and columns sync both ways
For a synced table, a change to a cell or a column on one side SHALL reach
the other through a three-way comparison with the base agreed at the last
sync. The same cell or the same column changed on both sides SHALL be raised
as a conflict, and SHALL NOT be sent until the member resolves it. A column
removed on one side and changed on the other SHALL be kept. Campbooks-only
columns SHALL never be sent. Notion formula, rollup, people and unsupported
columns SHALL be read-only, with Notion's values.

#### Scenario: Different cells
- **WHEN** Marta sets Value on a row in Campbooks and Rui sets Stage on the same row in Notion
- **THEN** after the next syncs both cells hold both changes, and no conflict is raised

#### Scenario: The same column renamed on both sides
- **WHEN** Marta renames Value to "Deal size" and Rui renames it to "Contract value" in Notion between two syncs
- **THEN** a column conflict offers both names, and neither rename is sent until Marta chooses

### Requirement: Removing a synced column is confirmed and restorable
Removing a column of a synced table SHALL require an explicit confirmation
that it is removed in Notion too. Without it the answer SHALL be 400
`confirmation_required`. For 30 days, `pages.restore_property` SHALL recreate
the column in both places and write its values back.

#### Scenario: Remove without confirming
- **WHEN** a client removes the Score column of a synced table without the confirmation
- **THEN** the response is 400 and the column stays in both places
