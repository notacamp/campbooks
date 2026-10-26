## ADDED Requirements

### Requirement: Pages say whether they are tables
With tables enabled, every page in `pages.list` and `pages.get` SHALL carry
its `kind`: `page` or `table`. Rows SHALL never be listed.

#### Scenario: The tree knows a table
- **WHEN** Marta lists her pages and "Clients" is a table
- **THEN** "Clients" comes back with kind `table`, and none of its rows are listed

### Requirement: An inline table block
A page body MAY hold a `tableView` block naming a table and one of its views
by id only. The server SHALL refuse a block naming a page that is not a table
or a view not of that table, and SHALL never copy the table's content into
the document. It SHALL record the block as an embed of the table, so the
table's backlinks show it. A reader who may not read the table SHALL get no
rows for it.

#### Scenario: An embedded table the reader cannot open
- **WHEN** Rui opens a page that embeds Marta's private "Clients" table
- **THEN** the block shows that something is there, and no row, title or value of the table reaches Rui

### Requirement: Rows can be read by id
`pages.query_table` SHALL take up to 100 row ids and answer only those of
that table that are live and readable, so a relation cell's rows and a row
opened as a page can be read without paging the whole table.

#### Scenario: A row of another table is not the table's
- **WHEN** Marta asks the Clients table for her Acme row and a row of Projects
- **THEN** only the Acme row comes back

### Requirement: The list says whether tables are on
`pages.list` SHALL say whether tables are on, so a client offers "New table",
`/table` and `/link to table` only when they work.

#### Scenario: Tables are off
- **WHEN** Marta lists her pages with tables off
- **THEN** the answer says so, and the client shows no table entry points
