## Purpose

Passkeys as a way in: registered while signed in, used to sign in, or used
alone to create an account when no provider is available.

## ADDED Requirements

### Requirement: Passkey registration
A signed-in person SHALL be able to register a passkey after a recent
sign-in. The server SHALL issue a single-use challenge valid for 5 minutes,
require user verification, accept the algorithms EdDSA, ES256 and RS256 with
attestation `none`, and verify the client data type, challenge, origin
(the instance's public origin) and relying-party id hash before storing the
credential's public key.

#### Scenario: Wrong origin
- **WHEN** a registration response carries an origin other than the instance's public origin
- **THEN** it is refused with `code` = `passkey_invalid` and nothing is stored

### Requirement: Passkey sign-in
The sign-in page SHALL offer passkeys through the browser's autofill
(conditional mediation) and an explicit button. An assertion SHALL be
accepted only for a stored credential, with a valid signature over the
authenticator data and client data hash, the user-present and
user-verified flags set, and a fresh single-use challenge. A signature
counter that goes backwards SHALL be recorded as an audit event.

#### Scenario: Replayed assertion
- **WHEN** an assertion is submitted a second time with the same challenge
- **THEN** it is refused and no session starts

### Requirement: Passkey-only sign-up
A person SHALL be able to create an account with a passkey and a display
name alone. The instance's signup policy SHALL still apply when that account
creates a workspace.

#### Scenario: No provider configured
- **WHEN** an instance has no provider app configured and a new person opens the sign-in page
- **THEN** they can create an account with a passkey and sign in with it afterwards
