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
