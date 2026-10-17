## ADDED Requirements

### Requirement: Signing key encoding
The server SHALL read an RSA signing key whose `n` or `e` is published with
leading zero bytes as the same number without them, for access tokens and
for identity providers' ID tokens alike. Every other check on the token
SHALL be unchanged.

#### Scenario: Zoho's published key
- **WHEN** a Zoho data centre publishes its signing key with `n` starting with a zero byte, and a person signs in with Zoho
- **THEN** the ID token's signature verifies against that key and the sign-in continues

#### Scenario: Signed by another key
- **WHEN** a token names that key's `kid` but was signed by a different key
- **THEN** it is refused as before
