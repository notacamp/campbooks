## ADDED Requirements

### Requirement: Revoked access tokens stop at once
Each account SHALL carry a revocation epoch (`tokens_not_before`), set to
the revocation's instant when the account is suspended or signs out
everywhere, in the same transaction, and never moved back (unsuspending
leaves it). The API SHALL check every person's access token of the
in-process issuer, on every verification, against the account through the
kernel's `TokenRevocation` port, and SHALL refuse it when the account does
not exist, is suspended, the token carries no `iat`, or its `iat` is not in
a later second than the epoch. A refused token SHALL get exactly the answer
of any invalid token (401, `code` `invalid_token`,
`WWW-Authenticate: Bearer error="invalid_token"`; 404 on the admin API),
saying nothing about the account. Every way a token is verified (scoped,
workspace and admin routes, the idempotency layer, the web BFF's proxied
calls) SHALL apply it. The token endpoint SHALL mint nothing
(`invalid_grant`) for a code or refresh token whose sign-in (`auth_time`)
is not newer than the epoch, and SHALL stamp a newer sign-in's access token
minted in the epoch's own second with the next second as `iat`, reading the
account under a share lock while minting. Client-credentials tokens and an
outside identity provider's tokens SHALL be verified as before.

#### Scenario: Suspended
- **WHEN** an account is suspended while a client holds one of its access tokens
- **THEN** that token gets 401 `invalid_token` on a scoped route and through the idempotency layer (a `POST` with an `Idempotency-Key`, replaying a key it used before), and 404 on the admin API

#### Scenario: Unsuspended
- **WHEN** the account is then unsuspended
- **THEN** the old token still gets 401, and a token from a new sign-in works

#### Scenario: Signed out everywhere
- **WHEN** a person signs out everywhere
- **THEN** the access tokens issued before get 401, and a token from a sign-in after it works

#### Scenario: The revocation's own second
- **WHEN** one token is minted earlier in the second of a revocation and another for a new sign-in later in that same second
- **THEN** the first is refused and the second is accepted

#### Scenario: A code from before the revocation
- **WHEN** a code handed out before a person signed out everywhere is redeemed afterwards
- **THEN** the token endpoint answers `invalid_grant`

#### Scenario: Not this issuer's people
- **WHEN** the API validates a workspace client's `client_credentials` token, or runs with an outside identity provider
- **THEN** no revocation is looked up and the token is accepted as before
