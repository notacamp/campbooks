## ADDED Requirements

### Requirement: Item summary
`ItemDetail` SHALL accept a `summary` (a model's plain sentences) and a
`summaryLabel`, and draw them as a region named by the label, marked with the
visor, after the reasons and above the email, as text only. It SHALL draw
nothing without a summary.

#### Scenario: A summary above the email
- **WHEN** an opened item has a summary and an email
- **THEN** the summary shows under its heading before the email, and without a summary only the email shows
