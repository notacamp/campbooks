# identity-authorization-server Specification

## Purpose
Campbooks as the OAuth 2.1 authorization server and OpenID provider of its
own API, on the app's origin, with keys it generates and rotates itself
(ADR 0014).

## Requirements

### Requirement: When the issuer runs in this process
The issuer SHALL run in the server when the instance has a secret key and
`CAMPBOOKS_AUTH__ISSUER` is unset or names `CAMPBOOKS_PUBLIC_URL`'s origin.
Otherwise every identity endpoint SHALL answer 404, and the configured
identity provider SHALL keep serving the instance. Outside
`CAMPBOOKS_ENV=development` the server SHALL refuse to start the issuer on a
plain `http` public URL, so its cookies are always `Secure` and
`__Host-`-prefixed.

#### Scenario: Another identity provider is configured
- **WHEN** `CAMPBOOKS_AUTH__ISSUER` names another identity provider and a client requests `/.well-known/openid-configuration`
- **THEN** the response is 404

#### Scenario: Plain http in production
- **WHEN** the issuer would run with `CAMPBOOKS_PUBLIC_URL=http://app.example.com` and `CAMPBOOKS_ENV=production`
- **THEN** the server refuses to start, naming `CAMPBOOKS_PUBLIC_URL`

### Requirement: Issuer metadata
The issuer SHALL be the instance's public origin. It SHALL publish OpenID
Connect Discovery and RFC 8414 metadata naming its endpoints (including
`registration_endpoint`), `S256` as the only code challenge method, the
grant types `authorization_code`, `refresh_token` and `client_credentials`,
`EdDSA` as the signing algorithm, the scopes it accepts, support for the
RFC 9207 `iss` response parameter, and
`client_id_metadata_document_supported: true`.

#### Scenario: Metadata advertises PKCE
- **WHEN** a client fetches `/.well-known/oauth-authorization-server`
- **THEN** `issuer` is the public origin and `code_challenge_methods_supported` is `["S256"]`

#### Scenario: Metadata names registration
- **WHEN** a client fetches `/.well-known/oauth-authorization-server`
- **THEN** `registration_endpoint` is `<origin>/oauth2/register` and `grant_types_supported` includes `client_credentials`

### Requirement: Authorization code with PKCE
`/oauth2/authorize` SHALL accept only `response_type=code` with an S256
`code_challenge`, and only scopes the client may ask for: the OIDC scopes,
and for Campbooks' own clients every scope of the catalogue, for any other
client the ones it registered and never an `identity:*` scope. It SHALL
match `redirect_uri` exactly against the client's registered values, except
that a loopback redirect matches on any port (RFC 8252 §7.3). For an unknown
client or an unregistered redirect it SHALL answer 400 and never redirect.
Every other error, and every code, SHALL go back to the client's redirect
URI with `state` and `iss`. A browser already signed in to the issuer SHALL
get a code at once for the web client, or go through the consent rules
below for any other client, unless the client sends `prompt=login` or a
`max_age` it exceeds; with `prompt=none` and no session the client SHALL get
`error=login_required`, and with consent to ask `error=consent_required`.
Codes SHALL be single-use and expire after 60 seconds; redeeming one again
SHALL fail and revoke every token issued from it.

#### Scenario: Plain PKCE refused
- **WHEN** a client sends `code_challenge_method=plain`
- **THEN** the browser is redirected back with `error=invalid_request`, `state` and `iss`, and no code

#### Scenario: Unregistered redirect
- **WHEN** a request names a `redirect_uri` the client did not register
- **THEN** the response is 400 with `code` = `invalid_redirect_uri` and no `Location`

#### Scenario: Code replay
- **WHEN** a code is redeemed a second time
- **THEN** the token request fails with `invalid_grant` and the refresh token issued at the first redemption stops working

#### Scenario: A third party asks for an identity scope
- **WHEN** a dynamically registered client asks for `identity:write`
- **THEN** the browser is redirected back with `error=invalid_scope`

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
API's resource URI, `<public origin>/api`), `client_id`, `azp`, `scope`,
`iat`, `nbf`, `exp` (ten minutes after `iat`), `jti`, `auth_time` for a
person's token, `workspace_id` when the token is bound to a workspace, and
the person's address and name when `email` and `profile` were granted. ID
tokens SHALL be signed the same way with `typ` = `JWT`, for the client,
carrying the request's `nonce` and the session's authentication methods
(`amr`, RFC 8176): `["fed"]` for a provider sign-in, `["hwk", "user"]` for a
device-bound passkey and `["swk", "user"]` for a synced one. The API SHALL
validate access tokens with the same verifier it uses for any issuer,
reading the keys in-process, SHALL expect the resource URI as `aud`, and
SHALL accept only `typ` = `at+jwt`
from the in-process issuer, so an ID token never passes as an access token.

#### Scenario: Token accepted by the API
- **WHEN** the web client calls the API with an access token from the issuer
- **THEN** the API accepts it and the handler sees the account id as the principal's subject

#### Scenario: An ID token is not an access token
- **WHEN** a client calls the API with an ID token of the in-process issuer whose `aud` matches the API's
- **THEN** the response is 401 with `code` = `invalid_token`

#### Scenario: A passkey sign-in's amr
- **WHEN** the web client redeems a code from a sign-in with a device-bound passkey
- **THEN** the ID token's `amr` is `["hwk", "user"]`, while one from a Google sign-in has `["fed"]`

### Requirement: Signing-key lifecycle
The server SHALL create a signing key the first time it needs one, exactly
once however many processes start together. It SHALL store private keys
sealed with a key derived from the instance secret key, publish the next key
seven days before it signs, rotate the signing key every 30 days, and keep a
retired key published for two days. When the signing key cannot be opened
with the instance secret key, the issuer SHALL fail closed: it SHALL sign
nothing, delete nothing, and the server SHALL refuse to start with an error
naming `CAMPBOOKS_SECRET_KEY`. No manual step SHALL be needed at install or
upgrade.

#### Scenario: Rotation without downtime
- **WHEN** 30 days pass after the first key was made
- **THEN** its successor, published for the past week, signs new tokens, and the first key stays in the JWKS for two more days

#### Scenario: Concurrent first start
- **WHEN** three processes ask for a signing key on an empty database at once
- **THEN** exactly one key exists and all three sign with it

#### Scenario: Another secret key
- **WHEN** a server starts with a `CAMPBOOKS_SECRET_KEY` other than the one the signing keys were sealed with
- **THEN** it refuses to start with an error naming `CAMPBOOKS_SECRET_KEY`, and the stored keys are untouched

### Requirement: The web BFF signs in against the in-process issuer
When the issuer runs in this process, the web BFF SHALL sign browsers in
against it as `campbooks-web`, with the derived secret and with its
discovery, token, revocation and end-session requests sent through the
issuer's router rather than the network. `/bff/logout` SHALL end the issuer
session itself, with the ID token it keeps as `id_token_hint`, and SHALL
give the browser no logout URL to visit.

#### Scenario: Web sign-in needs no provisioned secret
- **WHEN** a fresh install starts with `CAMPBOOKS_SECRET_KEY` and `CAMPBOOKS_PUBLIC_URL` only, and a person signs up with Google through `/bff/login`
- **THEN** they land in the app with a web session, and the BFF's API calls carry tokens the API accepts

#### Scenario: Signing out of the web app
- **WHEN** a signed-in person signs out of the web app and the app starts a new sign-in
- **THEN** `/bff/logout` answers no `logout_url` and the issuer shows its sign-in screen again

### Requirement: Revocation and logout
`/oauth2/revoke` SHALL revoke the sign-in a refresh token belongs to, for the
client that holds it, and SHALL answer 200 for unknown tokens (RFC 7009).
`/oauth2/end_session` (GET or form POST) SHALL require an `id_token_hint`
this issuer signed for one of its clients (expired is fine); without one it
SHALL answer 400 `invalid_id_token_hint` and end nothing. With one it SHALL
end the issuer session the hint names and that sign-in's refresh tokens,
and redirect only to a `post_logout_redirect_uri` the hint's client
registered.

#### Scenario: Revoked refresh token
- **WHEN** a client revokes its refresh token and then uses it
- **THEN** the refresh fails with `invalid_grant`

#### Scenario: Cross-site sign-out refused
- **WHEN** another site sends a browser to `/oauth2/end_session` without a valid `id_token_hint`
- **THEN** the response is 400 with `code` = `invalid_id_token_hint` and the person's tokens keep working
API's resource URI, `<public origin>/api`), `client_id`, `azp`, `scope`,
`iat`, `nbf`, `exp` (ten minutes after `iat`), `jti`, `auth_time` for a
person's token, `workspace_id` when the token is bound to a workspace, and
the person's address and name when `email` and `profile` were granted. ID
tokens SHALL be signed the same way with `typ` = `JWT`, for the client,
carrying the request's `nonce` and the session's authentication methods
(`amr`, RFC 8176): `["fed"]` for a provider sign-in, `["hwk", "user"]` for a
device-bound passkey and `["swk", "user"]` for a synced one. The API SHALL
validate access tokens with the same verifier it uses for any issuer,
reading the keys in-process, SHALL expect the resource URI as `aud`, and
SHALL accept only `typ` = `at+jwt`
