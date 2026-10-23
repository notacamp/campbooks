## ADDED Requirements

### Requirement: Ask texts
The `read-texts` v1 blob of a read SHALL also hold, when the read has asks,
each ask's key, title and amount, and SHALL be written when any item or ask
has text; the quote SHALL stay with the read. The event SHALL still carry no
text.

#### Scenario: An invoice with nothing to put on the calendar
- **WHEN** an invoice is read with one `pay` ask and no items
- **THEN** `message_read` points at a `read-texts` blob that holds the ask's title and amount
