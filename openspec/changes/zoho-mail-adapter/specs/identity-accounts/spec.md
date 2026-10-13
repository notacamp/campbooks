## ADDED Requirements

### Requirement: The first Zoho mailbox comes with sign-up
A Zoho round trip SHALL ask in one consent for `openid email profile`, the
mailbox scopes the grant-intake port reports for Zoho, and offline access,
and SHALL hold the grant of a new account as for Google. Granted scopes
SHALL be read split on spaces and commas; a Zoho token answer without
`scope` SHALL count as granting the mailbox scopes asked for.

#### Scenario: Sign up with Zoho, then first workspace
- **WHEN** a person signs up with Zoho on an instance with a Zoho mailbox adapter, creates a workspace and adopts the held grant
- **THEN** the grant-intake port receives the grant with its refresh token and the issuer `https://accounts.zoho.eu` for a European account

### Requirement: Grants name their issuer
Every grant identity hands to the grant-intake port, held or from a connect
or reconnect, SHALL carry the issuer that vouched for the account (the ID
token's `iss`, as validated), so the receiving module can reach the
account's data centre without trusting any other input.

#### Scenario: A held grant keeps its issuer
- **WHEN** a held Zoho grant from the EU data centre is adopted a day later
- **THEN** the grant carries the issuer `https://accounts.zoho.eu`
