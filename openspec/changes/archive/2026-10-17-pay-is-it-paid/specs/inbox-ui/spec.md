## ADDED Requirements

### Requirement: A bill in the reader
When the open conversation's need is a Pay question (trust `check`, question
`money`), the reader SHALL say that Campbooks can't see the bank and offer
Already paid (1) and Still to pay (2) in place of Yes and No, plus Not needed
(X), each acting as on Home, with Undo. A bill the member said is still to
pay SHALL offer Mark paid (recorded as Already paid) in place of Done.

#### Scenario: Still to pay from the reader
- **WHEN** Sam opens Acme Hosting's invoice and presses 2
- **THEN** the need becomes a sure Pay need and the reader offers Mark paid and Not needed
