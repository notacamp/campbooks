## ADDED Requirements

### Requirement: The receiving provider's authentication results
The mail adapters SHALL carry, with each message, the receiving provider's
own `Authentication-Results` (RFC 8601), unfolded: the topmost such header in
the message's own order, and only when it is valid UTF-8 and its authserv-id
is one the provider is known to stamp (Gmail: `mx.google.com`; Zoho Mail:
none until a real Zoho message has shown which header Zoho adds). Any other
`Authentication-Results`, including one a sender wrote into the message,
SHALL be dropped, and a message whose topmost one is not the provider's, or
is not valid UTF-8, SHALL carry none.

#### Scenario: Gmail's own results
- **WHEN** a Gmail mailbox receives a message whose topmost `Authentication-Results` is `mx.google.com; dkim=pass …; dmarc=pass … header.from=github.com`
- **THEN** the message change carries that value

#### Scenario: Results a sender wrote
- **WHEN** a message's topmost `Authentication-Results` names another server, or a Zoho mailbox receives one naming `mx.google.com` or `mx.zohomail.com`
- **THEN** the message change carries no authentication results

#### Scenario: A topmost header that is not UTF-8
- **WHEN** the topmost `Authentication-Results` holds a byte that is not UTF-8, above Gmail's own
- **THEN** the message change carries no authentication results
