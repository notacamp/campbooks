## ADDED Requirements

### Requirement: Only offers that take effect
The reader SHALL offer "Always show images from <sender>" and its ⇧A only
where the server says trusting the sender can take effect, with "Anyone at"
only for the domain the server offers; elsewhere, when remote images stay
held back, it SHALL say why in one line (under review, failing the sender's
checks, a provider without sender checks, or waiting for Scout's check). In
an inbox other members use, the note SHALL say the inbox trusts the sender.
The confirmation SHALL say that images wait for Scout's check, so they stay
held back while Scout is off or past its daily limit.

#### Scenario: Failing checks
- **WHEN** a message failing its sender's checks has its images held back
- **THEN** the reader says so in one line and offers no trust, and ⇧A does nothing

#### Scenario: A shared inbox
- **WHEN** a rule of a shared inbox trusts the Weekly Ledger
- **THEN** the note reads "this inbox trusts The Weekly Ledger"
