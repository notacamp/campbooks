## ADDED Requirements

### Requirement: A copied recipient is a hint, never a decision
The facts SHALL note whether one of the inbox's addresses is in `Cc` and none
is in `To` (addresses compared without regard to case; no hint when the inbox
is unknown). The note SHALL reach the `decide` model as a hint, and the facts
strategy SHALL NOT answer any question because of it: whether a copied person
is asked something directly is for the model to read.

#### Scenario: Copied only
- **WHEN** the inbox's address is in `Cc` and not in `To`
- **THEN** the hint is set and `needs_reply` is still not answered by the facts

#### Scenario: Addressed
- **WHEN** the inbox's address is in `To`, also in `Cc` or not in either
- **THEN** the hint is not set
