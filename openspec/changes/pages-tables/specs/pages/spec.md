## ADDED Requirements

### Requirement: A table and its rows
With `CAMPBOOKS_PAGES__TABLES__ENABLED` on, a page MAY be a table. A
table's rows SHALL be pages of kind `row` under it, so they follow the
table's access, trash, search, mentions and backlinks, and can each hold a
page body. A table SHALL have exactly one title property and at least one
view, and SHALL hold at most 200 properties and 20,000 rows.

#### Scenario: A new table
- **WHEN** Marta creates the table "Clients"
- **THEN** it has a title property, one table view, and no rows

#### Scenario: A reader edits a cell
- **WHEN** Rui, a reader of "Clients", sets a cell
- **THEN** the response is 403 and the cell is unchanged

### Requirement: Typed values
Every property SHALL have one of the types in design.md, and every value
SHALL be checked against its property's type and config before it is
stored. Select, multi-select and status values SHALL be stored by option
id. A paired relation SHALL be updated on both sides in the same
transaction. A Campbooks link SHALL store only references, rendered through
each record's own module with the viewer's access. The read-only types
reserved for Notion SHALL NOT be created by members.

#### Scenario: Renaming an option
- **WHEN** Marta renames the Stage option "Kickoff" to "Onboarding"
- **THEN** every row that had "Kickoff" shows "Onboarding", and no row value changes

#### Scenario: A bad value
- **WHEN** a client sets a number cell to "lots"
- **THEN** the response is 400 and nothing is stored

### Requirement: Cells never collide
Each cell SHALL carry its own version. Writes to different cells of a row
SHALL never conflict. A write to a cell that changed since the writer read
it SHALL still be stored, and SHALL answer the value it replaced so the
writer can undo.

#### Scenario: Two people, two cells
- **WHEN** Marta sets Stage and Rui sets Value on the same row at the same time
- **THEN** both values are stored

### Requirement: Querying a table
`pages.query_table` SHALL answer the rows the caller may read for a view or
an ad-hoc filter and sort: filters in AND/OR groups up to three levels, up
to five sorts, 100 rows per page with a cursor, and the view's column
summaries. It SHALL be computed in the database with casts by type.

#### Scenario: Filter and sum
- **WHEN** the "All clients" view filters Stage is not Paused and sums Value
- **THEN** the answer lists only rows not Paused, and the sum of their values

### Requirement: Views
A table SHALL have table and board views with filters, sorts, visible
properties, order, widths and summaries. A board SHALL group by a select or
status property. Moving a card to another group SHALL set that row's value,
with undo. View types `list`, `calendar` and `gallery` SHALL be reserved and
refused in this version.

#### Scenario: Dragging a card
- **WHEN** Marta drags "Northwind Bakery" from "Proposal sent" to "Kickoff" on the board
- **THEN** its Stage becomes Kickoff and an Undo toast is shown
