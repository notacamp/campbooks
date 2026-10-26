## ADDED Requirements

### Requirement: Zoho always shows its screen for a mailbox
An authorization request to Zoho that asks for a mailbox (connect or reconnect)
SHALL carry `prompt=consent`, whether or not consent was otherwise requested
again, so Zoho shows a screen and the member sees which account answers. Google
and Microsoft requests SHALL be unchanged.

#### Scenario: Zoho reconnect
- **WHEN** a member reconnects a Zoho mailbox
- **THEN** the authorization URL carries `prompt=consent` and the mailbox's `login_hint`
