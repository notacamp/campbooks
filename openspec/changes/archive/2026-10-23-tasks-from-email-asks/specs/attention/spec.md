## MODIFIED Requirements

### Requirement: A task replaces its conversation's item
`attention.list` SHALL leave out a conversation's `do` or `pay` item for a
member who has a task item from that conversation that is open, or that was
cleared at or after the conversation item's `since`.

#### Scenario: Made a task from the lease email
- **WHEN** Ana makes a task from a conversation that has a Do need
- **THEN** Home lists the task once and does not list the conversation's Do item

#### Scenario: Paid
- **WHEN** Ana marks the task from the EDP invoice paid
- **THEN** neither the task nor the conversation's Pay item is on Home

#### Scenario: A new bill in the same thread
- **WHEN** after Ana paid, a new EDP invoice arrives in that conversation and opens a newer Pay item
- **THEN** Home shows that Pay item again until its own task is made
