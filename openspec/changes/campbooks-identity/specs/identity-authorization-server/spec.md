## Purpose

Campbooks as the OAuth 2.1 authorization server and OpenID provider of its
own API, on the app's origin, with keys it generates and rotates itself.

## ADDED Requirements

### Requirement: Issuer metadata
The issuer SHALL be the instance's public URL (`CAMPBOOKS_PUBLIC_URL`) unless
`CAMPBOOKS_AUTH__ISSUER` overrides it. It SHALL publish OpenID Connect
Discovery and RFC 8414 metadata naming its endpoints, `S256` as the only code
challenge method, the grant types `authorization_code`, `refresh_token` and
`client_credentials`, `EdDSA` as the signing algorithm, and the scopes of the
catalogue. It SHALL publish RFC 9728 protected-resource metadata for the API.

#### Scenario: Metadata advertises PKCE
- **WHEN** a client fetches `/.well-known/oauth-authorization-server`
- **THEN** `issuer` equals the public URL and `code_challenge_methods_supported` is `["S256"]`

### Requirement: Authorization code with PKCE
`/oauth2/authorize` SHALL accept only `response_type=code` with a
`code_challenge` and `code_challenge_method=S256`. It SHALL match
`redirect_uri` exactly against the client's registered values, except the
port of a loopback redirect for native clients. For an unknown client or an
unregistered redirect it SHALL show an error and never redirect. Every
authorization response SHALL carry `iss`. Authorization codes SHALL be
single-use and expire after 60 seconds; redeeming a code a second time SHALL
fail and SHALL revoke every token issued from it.

#### Scenario: Plain PKCE refused
- **WHEN** a client sends `code_challenge_method=plain`
- **THEN** the browser is redirected back with `error=invalid_request` and no code

#### Scenario: Unregistered redirect
- **WHEN** a request names a `redirect_uri` the client did not register
- **THEN** an error page is shown and the browser is not redirected anywhere

#### Scenario: Code replay
- **WHEN** a code is redeemed a second time
- **THEN** the token request fails with `invalid_grant` and the refresh token issued at the first redemption stops working

### Requirement: Refresh-token rotation with reuse detection
Every refresh SHALL return a new refresh token and invalidate the one used.
Presenting a refresh token that was already rotated SHALL fail with
`invalid_grant`, revoke every token of its family, and record a
`identity.refresh_reuse_detected` event.

#### Scenario: Stolen refresh token replayed
- **WHEN** refresh token R1 is used to obtain R2 and then R1 is presented again
- **THEN** the request fails with `invalid_grant` and R2 no longer works

### Requirement: Access tokens
Access tokens SHALL be JWTs with header `typ` = `at+jwt`, signed with an
Ed25519 key published in `/oauth2/jwks`, carrying `iss`, `sub`, `aud` (the
requested resource, by default the API's canonical URI), `client_id`,
`scope`, `iat`, `nbf`, `exp` (at most 10 minutes after `iat`) and `jti`, and
`workspace_id` when the token is bound to a workspace. A `resource` value the
instance does not serve SHALL be refused with `invalid_target`. The API SHALL
validate these tokens with the same verifier it uses for any issuer, reading
the keys in-process.

#### Scenario: Token accepted by the API
- **WHEN** the web BFF calls the API with an access token from the issuer
- **THEN** the API's verifier validates it and the handler sees the account id as the principal subject

#### Scenario: Unknown resource
- **WHEN** a token request names `resource=https://other.example.com`
- **THEN** it fails with `invalid_target`

### Requirement: Signing-key lifecycle
The server SHALL create a signing key at first start when none is active,
exactly once however many processes start together. It SHALL store private
keys sealed with a key derived from the instance secret key. It SHALL publish
the next key at least 7 days before it becomes active, rotate the active key
every 30 days, and keep a retired key published for at least 2 days. No
manual step SHALL be needed at install or upgrade.

#### Scenario: Rotation without downtime
- **WHEN** the active key is rotated
- **THEN** tokens signed with the previous key keep validating until they expire, and the new key was in the JWKS before it signed anything

#### Scenario: Concurrent first start
- **WHEN** three server processes start against an empty database at once
- **THEN** exactly one active signing key exists

### Requirement: The web BFF signs in against the in-process issuer
The web BFF SHALL sign browsers in with the authorization code flow against
the in-process issuer as the first-party client `campbooks-web`, whose secret
is derived from the instance secret key. No consent screen SHALL be shown to
first-party clients. `/bff/logout` SHALL also end the issuer's browser
session, with no confirmation page.

#### Scenario: Web sign-in needs no provisioned secret
- **WHEN** a fresh install starts with `CAMPBOOKS_SECRET_KEY` and `CAMPBOOKS_PUBLIC_URL` only
- **THEN** a browser can sign in through `/bff/login`

### Requirement: Revocation and logout
`/oauth2/revoke` SHALL revoke a refresh token's family or mark an access
token's `jti` revoked for its remaining lifetime (RFC 7009), answering 200
for unknown tokens. `/oauth2/end_session` SHALL end the issuer session for a
valid `id_token_hint` or first-party session and redirect only to a
registered post-logout URI.

#### Scenario: Revoked refresh token
- **WHEN** a client revokes its refresh token and then uses it
- **THEN** the refresh fails with `invalid_grant`
