## Purpose

Campbooks as the OAuth 2.1 authorization server and OpenID provider of its
own API, on the app's origin, with keys it generates and rotates itself
(ADR 0014).

## ADDED Requirements

### Requirement: When the issuer runs in this process
The issuer SHALL run in the server when the instance has a secret key and
`CAMPBOOKS_AUTH__ISSUER` is unset or names `CAMPBOOKS_PUBLIC_URL`'s origin.
Otherwise every identity endpoint SHALL answer 404, and the configured
identity provider SHALL keep serving the instance.

#### Scenario: Another identity provider is configured
- **WHEN** `CAMPBOOKS_AUTH__ISSUER` names another identity provider and a client requests `/.well-known/openid-configuration`
- **THEN** the response is 404

### Requirement: Issuer metadata
The issuer SHALL be the instance's public origin. It SHALL publish OpenID
Connect Discovery and RFC 8414 metadata naming its endpoints, `S256` as the
only code challenge method, the grant types `authorization_code` and
`refresh_token`, `EdDSA` as the signing algorithm, the scopes it accepts, and
support for the RFC 9207 `iss` response parameter.

#### Scenario: Metadata advertises PKCE
- **WHEN** a client fetches `/.well-known/oauth-authorization-server`
- **THEN** `issuer` is the public origin and `code_challenge_methods_supported` is `["S256"]`

### Requirement: First-party clients without provisioning
The web client `campbooks-web` (confidential, its secret derived from the
instance secret key) and the native client `campbooks-native` (public, PKCE
only, loopback redirects on any port and the private-use scheme
`com.notacamp.campbooks:/oauth/callback`) SHALL exist on every install with
no provisioning step, and SHALL get no consent screen.

#### Scenario: Native loopback on any port
- **WHEN** the native client sends `redirect_uri=http://127.0.0.1:53124/callback` for a signed-in browser
- **THEN** the browser is redirected there with a code, and the code is redeemed without a client secret

### Requirement: Authorization code with PKCE
`/oauth2/authorize` SHALL accept only `response_type=code` with an S256
`code_challenge`, and only scopes it knows. It SHALL match `redirect_uri`
exactly against the client's registered values (except the loopback port of
the native client). For an unknown client or an unregistered redirect it
SHALL answer 400 and never redirect. Every other error, and every code,
SHALL go back to the client's redirect URI with `state` and `iss`. A browser
already signed in to the issuer SHALL get a code at once unless the client
sends `prompt=login` or a `max_age` it exceeds; with `prompt=none` and no
session the client SHALL get `error=login_required`. Codes SHALL be
single-use and expire after 60 seconds; redeeming one again SHALL fail and
revoke every token issued from it.

#### Scenario: Plain PKCE refused
- **WHEN** a client sends `code_challenge_method=plain`
- **THEN** the browser is redirected back with `error=invalid_request`, `state` and `iss`, and no code

#### Scenario: Unregistered redirect
- **WHEN** a request names a `redirect_uri` the client did not register
- **THEN** the response is 400 with `code` = `invalid_redirect_uri` and no `Location`

#### Scenario: Code replay
- **WHEN** a code is redeemed a second time
- **THEN** the token request fails with `invalid_grant` and the refresh token issued at the first redemption stops working

### Requirement: Client authentication at the token endpoint
Confidential clients SHALL authenticate with `client_secret_basic` or
`client_secret_post`, compared in constant time; public clients SHALL send
only `client_id`. A failure SHALL answer 401 `invalid_client`. Token
responses SHALL be `Cache-Control: no-store`.

#### Scenario: Wrong secret
- **WHEN** the web client redeems a code with a wrong secret
- **THEN** the response is 401 with `error` = `invalid_client`

### Requirement: Refresh-token rotation with reuse detection
A refresh token SHALL be issued when `offline_access` was granted. Every
refresh SHALL return a new refresh token and invalidate the one used.
Presenting a rotated refresh token SHALL fail with `invalid_grant`, revoke
every token of its sign-in, and record `identity.refresh_reuse_detected`.

#### Scenario: A replayed refresh token
- **WHEN** refresh token R1 is used to obtain R2 and then R1 is presented again
- **THEN** the request fails with `invalid_grant` and R2 no longer works

### Requirement: Access and ID tokens
Access tokens SHALL be JWTs with header `typ` = `at+jwt`, signed with an
Ed25519 key published in `/oauth2/jwks`, carrying `iss`, `sub`, `aud` (the
configured audience), `client_id`, `azp`, `scope`, `iat`, `nbf`, `exp` (ten
minutes after `iat`), `jti` and `auth_time`, and the person's address and
name when `email` and `profile` were granted. ID tokens SHALL be signed the
same way, for the client, carrying the request's `nonce`. The API SHALL
validate access tokens with the same verifier it uses for any issuer,
reading the keys in-process.

#### Scenario: Token accepted by the API
- **WHEN** the web client calls the API with an access token from the issuer
- **THEN** the API accepts it and the handler sees the account id as the principal's subject

### Requirement: Signing-key lifecycle
The server SHALL create a signing key the first time it needs one, exactly
once however many processes start together. It SHALL store private keys
sealed with a key derived from the instance secret key, publish the next key
seven days before it signs, rotate the signing key every 30 days, and keep a
retired key published for two days. A key that cannot be opened (the secret
key changed) SHALL be replaced. No manual step SHALL be needed at install or
upgrade.

#### Scenario: Rotation without downtime
- **WHEN** 30 days pass after the first key was made
- **THEN** its successor, published for the past week, signs new tokens, and the first key stays in the JWKS for two more days

#### Scenario: Concurrent first start
- **WHEN** three processes ask for a signing key on an empty database at once
- **THEN** exactly one key exists and all three sign with it

### Requirement: The web BFF signs in against the in-process issuer
When the issuer runs in this process, the web BFF SHALL sign browsers in
against it as `campbooks-web`, with the derived secret and with its
discovery, token and revocation requests sent through the issuer's router
rather than the network. `/bff/logout` SHALL lead to `/oauth2/end_session`,
which ends the issuer session and its refresh tokens and returns to the
app's root with no confirmation page.

#### Scenario: Web sign-in needs no provisioned secret
- **WHEN** a fresh install starts with `CAMPBOOKS_SECRET_KEY` and `CAMPBOOKS_PUBLIC_URL` only, and a person signs up with Google through `/bff/login`
- **THEN** they land in the app with a web session, and the BFF's API calls carry tokens the API accepts

### Requirement: Revocation
`/oauth2/revoke` SHALL revoke the sign-in a refresh token belongs to, for the
client that holds it, and SHALL answer 200 for unknown tokens (RFC 7009).

#### Scenario: Revoked refresh token
- **WHEN** a client revokes its refresh token and then uses it
- **THEN** the refresh fails with `invalid_grant`
