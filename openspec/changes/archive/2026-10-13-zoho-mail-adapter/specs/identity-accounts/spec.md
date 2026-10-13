## ADDED Requirements

### Requirement: The first Zoho mailbox comes with sign-up
A Zoho sign-in SHALL ask for `openid email profile` only. When it turns
out to be a new account and the instance has a Zoho mailbox adapter, the
browser SHALL be sent to Zoho once more, for that account, asking for the
mailbox scopes and offline access with `prompt=consent`, and the grant SHALL
be held as for Google. A connect or reconnect SHALL ask for the mailbox at
once. A refresh token Campbooks does not keep (a known account's sign-in, a
refused link or connect, a discarded held grant) SHALL be revoked at Zoho,
which caps the tokens a person holds per app. Granted scopes
SHALL be read split on spaces and commas; a Zoho token answer without
`scope` SHALL count as granting the mailbox scopes asked for.

#### Scenario: A known Zoho account signs in
- **WHEN** a person whose Zoho account is a way in signs in with Zoho
- **THEN** the authorization request asks for no mailbox and no offline access, and nothing is held

#### Scenario: A refused connect
- **WHEN** a Zoho connect is refused because that Zoho account is another person's way in
- **THEN** the refresh token Zoho issued is revoked

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
