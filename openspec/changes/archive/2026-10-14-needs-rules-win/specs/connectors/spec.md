## ADDED Requirements

### Requirement: The receiving provider's authentication results
The mail adapters SHALL carry, with each message, the receiving provider's
own `Authentication-Results` (RFC 8601), unfolded: the topmost such header,
and only when its authserv-id is the provider's (Gmail: `mx.google.com`;
Zoho Mail: its regional mail exchangers). Any other `Authentication-Results`,
including one a sender wrote into the message, SHALL be dropped, and a
message whose topmost one is not the provider's SHALL carry none.

#### Scenario: Gmail's own results
- **WHEN** a Gmail mailbox receives a message whose topmost `Authentication-Results` is `mx.google.com; dkim=pass …; dmarc=pass … header.from=github.com`
- **THEN** the message change carries that value

#### Scenario: Results a sender wrote
- **WHEN** a message's topmost `Authentication-Results` names another server, or a Zoho mailbox receives one naming `mx.google.com`
- **THEN** the message change carries no authentication results
