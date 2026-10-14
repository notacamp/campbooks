## ADDED Requirements

### Requirement: Suspended accounts get in by no door
A suspended account SHALL be refused at every way into the issuer: a
provider sign-in SHALL send the browser to the sign-in screen with
`error=account_suspended` and start no session; a passkey sign-in SHALL
answer 403 `account_suspended` once the assertion verified (and
`passkey_not_accepted` before), starting no session; its issuer sessions SHALL no
longer sign it in to any client; its authorization codes and refresh tokens
SHALL be refused with `invalid_grant`. Once unsuspended, the account SHALL
sign in again with its ways in; nothing ended by the suspension is restored.

#### Scenario: Every door refuses
- **WHEN** an account that holds a refresh token, an unredeemed code and an issuer session is suspended
- **THEN** the refresh token and the code answer `invalid_grant`, `/oauth2/authorize` opens the sign-in screen instead of returning a code, and signing in with Google comes back with `error=account_suspended`

#### Scenario: Passkey of a suspended account
- **WHEN** a suspended account's passkey signs in with a valid assertion
- **THEN** the answer is 403 `account_suspended` and no session starts, while a tampered signature for the same passkey answers `passkey_not_accepted`

#### Scenario: Unsuspended
- **WHEN** the account is unsuspended and the person signs in with Google
- **THEN** the web client receives a code and redeems it for tokens
