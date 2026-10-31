## ADDED Requirements

### Requirement: Table cells that do not fit end in an ellipsis
A table's cells SHALL show their content on one
line, ending in an ellipsis when the column is too narrow, and SHALL carry the
whole value in their title (a footer summary wraps instead, with the whole value in its title). The add-column control SHALL sit in the header row,
and the footer's "New row" SHALL show a single "+" at any width.

#### Scenario: A narrow number column
- **WHEN** the Value column is as narrow as it can be and a cell holds €9,600.00
- **THEN** the cell ends in an ellipsis and its title is "€9,600.00"

#### Scenario: New row on a phone
- **WHEN** Ana opens a table at 375 px
- **THEN** "New row" shows one "+", and "Add a column" is in the header and can be clicked
