## ADDED Requirements

### Requirement: The table page
A table page SHALL show its views as tabs, a one-row toolbar (filter chips,
Filter, Sort, New), and a grid whose cells edit in place with an editor for
each type. It SHALL support arrow keys to move, Enter to edit, Esc to
cancel, ⌘↵ to open the row and `n r` for a new row, and SHALL show footer
summaries. A new column SHALL be added from a type menu grouped as Basic,
Links and Filled in for you.

#### Scenario: Adding a column
- **WHEN** Marta adds a Campbooks link column named "Threads"
- **THEN** the column appears, and each cell can link threads picked by name

### Requirement: Board and row peek
The board SHALL show one column per group with its count and summary, and
cards that open the row. Opening a row SHALL show a side panel with its
properties and its page body, and "Open as page".

#### Scenario: Opening a row
- **WHEN** Marta opens "Acme Studio" from the board
- **THEN** a side panel shows its Stage, Owner, Next date, Value, Threads and its notes

### Requirement: Inline tables and phone layout
`/table` SHALL make a new inline table in a page, and `/link to table` SHALL
embed a view of an existing table. A reader without access to the table SHALL
see "Something you can't open". Grids and boards SHALL scroll inside their
own box, so a page never scrolls sideways, at 375 px and 1280 px in every
locale.

#### Scenario: A table on a phone
- **WHEN** "Clients" is opened at 375 px
- **THEN** the grid scrolls sideways inside its box, and the page does not

### Requirement: The address holds the view and the row
A table page's current view and the row opened beside it SHALL be in its
address, so a link opens them and Escape or Close takes the row out again.

#### Scenario: A link to a row
- **WHEN** Rui opens the link to the Acme row of the Clients board
- **THEN** the board opens with the Acme row beside it

### Requirement: Every change can be undone
Every change to a cell, a title, a row, a column, an option or a view SHALL
show an Undo, and a failure SHALL roll the cell back and say so until Retry.
A cell somebody else changed meanwhile SHALL be said in the toast, with the
member's value kept.

#### Scenario: A concurrent edit
- **WHEN** Marta saves a Value that Rui changed a moment before
- **THEN** the toast says someone else changed it too and that hers is kept, with Undo
