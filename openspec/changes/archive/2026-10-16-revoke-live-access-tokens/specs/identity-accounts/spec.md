## MODIFIED Requirements

### Requirement: Suspended accounts get in by no door
A suspended account SHALL be refused at every way into the issuer: a
provider sign-in SHALL send the browser to the sign-in screen with
`error=account_suspended` and start no session; a passkey sign-in SHALL
answer 403 `account_suspended` once the assertion verified (and
`passkey_not_accepted` before), starting no session; its issuer sessions SHALL no
longer sign it in to any client; its authorization codes and refresh tokens
SHALL be refused with `invalid_grant`; the access tokens already issued to it
SHALL be refused by the API at once (see "Revoked access tokens stop at
once"). Once unsuspended, the account SHALL sign in again with its ways in;
nothing ended by the suspension is restored, its old access tokens
included.

#### Scenario: Every door refuses
- **WHEN** an account that holds a refresh token, an unredeemed code and an issuer session is suspended
- **THEN** the refresh token and the code answer `invalid_grant`, `/oauth2/authorize` opens the sign-in screen instead of returning a code, and signing in with Google comes back with `error=account_suspended`

#### Scenario: Passkey of a suspended account
- **WHEN** a suspended account's passkey signs in with a valid assertion
- **THEN** the answer is 403 `account_suspended` and no session starts, while a tampered signature for the same passkey answers `passkey_not_accepted`

#### Scenario: Unsuspended
- **WHEN** the account is unsuspended and the person signs in with Google
- **THEN** the web client receives a code and redeems it for tokens

#### Scenario: Access tokens already issued
- **WHEN** an account is suspended while a client holds one of its access tokens, and is then unsuspended
- **THEN** the token gets 401 from the moment of the suspension and still after the unsuspension

### Requirement: Signing out everywhere
`identity.sign_out_everywhere` (`identity:write`, `POST
/api/v1/identity/sign-out-everywhere`) SHALL end every live issuer session of
the caller's account and revoke every refresh token it holds, for every
client, make the API refuse every access token issued to the account so
far (see "Revoked access tokens stop at once"), record it in the audit
trail and publish `identity.signed_out_everywhere`; the web BFF SHALL end the account's web
sessions on it. It SHALL need no recent sign-in. A token bound to a workspace
SHALL get 404 and end nothing. Ways to sign in SHALL offer it behind an
inline confirmation, then sign the browser out.

#### Scenario: A stolen session
- **WHEN** a person signs out everywhere while another browser holds a session and a refresh token of their account
- **THEN** that refresh token fails with `invalid_grant`, the access tokens issued before get 401 at once, that browser's next authorization shows the sign-in screen, and its web session ends
