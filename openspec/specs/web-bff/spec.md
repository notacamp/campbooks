# web-bff Specification

## Purpose
How a web browser signs in to Campbooks and calls the API through the
server's backend-for-frontend without ever holding a token (ADR 0004).

## Requirements

### Requirement: Web sign-in configuration
The server SHALL offer web sign-in only when an identity provider is
configured (`CAMPBOOKS_AUTH__ISSUER`) and the web client's secret is available
(`CAMPBOOKS_BFF__CLIENT_SECRET_FILE` or `CAMPBOOKS_BFF__CLIENT_SECRET`). When
web sign-in is configured, the server SHALL refuse to start without an
instance secret key (`CAMPBOOKS_SECRET_KEY` or `CAMPBOOKS_SECRET_KEY_FILE`,
32 bytes, base64) and SHALL name the setting. When web sign-in is not
configured, every `/bff/*` endpoint except `/bff/session` SHALL answer 503 as
`application/problem+json` with `code` = `bff_not_configured`.

#### Scenario: No identity provider
- **WHEN** `CAMPBOOKS_AUTH__ISSUER` is unset and a browser requests `/bff/login`
- **THEN** the response is 503 with `code` = `bff_not_configured`

#### Scenario: Missing secret key
- **WHEN** the server starts with an issuer and a client secret but no `CAMPBOOKS_SECRET_KEY`
- **THEN** it exits with a non-zero status and an error naming `CAMPBOOKS_SECRET_KEY`

### Requirement: Starting a sign-in
`GET /bff/login` SHALL redirect (303) to the identity provider's authorization
endpoint with `response_type=code`, the web client id, the redirect URI
`<public URL>/bff/callback`, the configured scopes, a fresh random `state` and
`nonce`, and a PKCE `code_challenge` with `code_challenge_method=S256`. It
SHALL remember the sign-in server-side for a limited time and bind it to the
browser with an `HttpOnly`, `SameSite=Lax` login cookie. The optional
`return_to` SHALL be a path on this site; anything else SHALL be refused with
400 `invalid_return_to`.

#### Scenario: Redirect to the identity provider
- **WHEN** a browser requests `/bff/login?return_to=/inbox`
- **THEN** the response is 303 to the authorization endpoint carrying state, nonce and an S256 code challenge, and sets the login cookie

#### Scenario: Open redirect refused
- **WHEN** `return_to` is `//evil.example.com`, an absolute URL, or contains a backslash
- **THEN** the response is 400 with `code` = `invalid_return_to` and no sign-in is started

### Requirement: Finishing a sign-in
`GET /bff/callback` SHALL accept a `state` only once, only before the sign-in
expires, and only from the browser holding that sign-in's login cookie;
otherwise it SHALL answer 400 `invalid_state`. It SHALL refuse an IdP error or
a response naming another issuer (`login_failed`), exchange the code with the
PKCE verifier, and validate the ID token's signature, issuer, audience (the web
client), `azp`, expiry and nonce (`invalid_id_token` otherwise). On success it
SHALL end any session the browser already had, start a new session, set the
session cookie, clear the login cookie and answer 200 with a same-origin page
that moves on to `return_to`, never with a redirect.

#### Scenario: Successful sign-in
- **WHEN** the identity provider redirects back with a valid code and the state of a sign-in started by this browser
- **THEN** the response is 200 HTML that navigates to `return_to`, sets the session cookie and stores no token in it

#### Scenario: Replayed or foreign state
- **WHEN** the same callback URL is used a second time, or by a browser that did not start the sign-in
- **THEN** the response is 400 with `code` = `invalid_state` and no session is created

#### Scenario: Nonce mismatch
- **WHEN** the ID token's nonce differs from the sign-in's
- **THEN** the response is 400 with `code` = `invalid_id_token` and no session is created

### Requirement: Session cookie
The session cookie SHALL hold only an opaque random value of at least 256 bits
and SHALL be `HttpOnly`, `SameSite=Strict` and `Path=/` with no `Domain`. When
the public URL is https it SHALL be named `__Host-campbooks_session` and be
`Secure`; when the public URL is plain http, it SHALL be named
`campbooks_session` without `Secure`, and the server SHALL log this once at
startup. Browser JavaScript SHALL NOT be able to read it.

#### Scenario: Cookie attributes over https
- **WHEN** a sign-in completes on an instance whose public URL is https
- **THEN** the `Set-Cookie` is `__Host-campbooks_session=…; Path=/; Max-Age=…; HttpOnly; SameSite=Strict; Secure`

### Requirement: Session store
Sessions SHALL be stored in Postgres schema `bff` with the cookie value only
as a hash, and the access, refresh and ID tokens encrypted with a key derived
from the instance secret key. A session SHALL end when unused for the idle
timeout, at the absolute timeout, on logout or on a back-channel logout. Its
cookie value SHALL be replaced periodically, the previous value staying valid
for a short grace period. Ended sessions SHALL be deleted by a scheduled job.
A session whose tokens can no longer be decrypted SHALL end.

#### Scenario: Idle session
- **WHEN** a session has not been used for longer than the idle timeout
- **THEN** `/bff/session` answers `{ "authenticated": false }` and clears the cookie

#### Scenario: Rotation
- **WHEN** a session's cookie value is older than the rotation interval and the browser makes a request
- **THEN** the response sets a new cookie value, and requests still carrying the old value succeed during the grace period only

#### Scenario: Pruning
- **WHEN** the prune job runs
- **THEN** sessions past their idle or absolute deadline and expired sign-ins are deleted, and live ones are kept

### Requirement: Current session
`GET /bff/session` SHALL answer 200 with `authenticated`, and when
authenticated the user's `sub`, name and email and when the session ends. It
SHALL never include an access, refresh or ID token or the session id.

#### Scenario: Signed in
- **WHEN** a signed-in browser requests `/bff/session`
- **THEN** the body is `{ "authenticated": true, "user": { "sub", "name", "email" }, "expires_at" }` and contains no token

### Requirement: API access through the BFF
A request to `/bff/api/<path>` with a live session SHALL be handled as
`/api/<path>` (same method, query, headers and body) by the API in the same
process, with the session's access token as `Authorization: Bearer`, and
without the browser's cookies, `Authorization` or CSRF headers. Without a live
session it SHALL answer 401 `session_required`. The access token SHALL be
refreshed with the refresh token shortly before it expires, never earlier
than the identity provider allows; concurrent requests of one session SHALL
cause one refresh. A refused refresh SHALL end the session.

#### Scenario: Token attached
- **WHEN** a signed-in browser requests `/bff/api/v1/<route>` of a scoped endpoint
- **THEN** the API sees the signed-in user as the principal and the browser never sees the token

#### Scenario: Refresh once under concurrency
- **WHEN** several requests of one session arrive while its access token is about to expire
- **THEN** exactly one refresh is made, all requests succeed, and the rotated refresh token is stored

#### Scenario: No session
- **WHEN** a browser without a live session requests `/bff/api/v1/system/version`
- **THEN** the response is 401 with `code` = `session_required`

### Requirement: CSRF protection
Every request with an unsafe method (anything but GET, HEAD, OPTIONS, TRACE) to
a cookie-authenticated BFF endpoint SHALL carry `X-Campbooks-CSRF: 1` and
either `Sec-Fetch-Site: same-origin` or, when no `Sec-Fetch-Site` is sent, an
`Origin` equal to the public URL's origin. Other unsafe requests SHALL be
refused with 403 `csrf_check_failed` before any other processing.

#### Scenario: Cross-site write refused
- **WHEN** a request to `/bff/api/*` or `/bff/logout` uses POST without the header, or with `Sec-Fetch-Site: cross-site`, or with a foreign `Origin`
- **THEN** the response is 403 with `code` = `csrf_check_failed`

### Requirement: Logout
`POST /bff/logout` SHALL end the session, revoke its refresh token at the
identity provider, clear the session cookie and answer with the identity
provider's end-session URL (carrying the client id and the post-logout
redirect URI, and no token), or `null` when the provider has none. It SHALL
answer the same way when there is no session.

#### Scenario: Sign out
- **WHEN** a signed-in browser posts to `/bff/logout` with the CSRF header
- **THEN** the session is gone, its refresh token is revoked, the cookie is cleared and the body has the end-session URL

### Requirement: Back-channel logout
`POST /bff/backchannel-logout` SHALL accept a logout token from the identity
provider (OIDC Back-Channel Logout 1.0) and, when it validates (signature,
issuer, audience, `iat`, `exp`, the back-channel event, no nonce, a `sub`
and/or `sid`), end the sessions of that identity-provider session, or every
session of the user when only `sub` is given. An invalid token SHALL be
refused with 400 `invalid_logout_token` and end nothing.

#### Scenario: Identity-provider session ends
- **WHEN** the identity provider posts a valid logout token naming `sid`
- **THEN** the web sessions started in that identity-provider session end, and others of the same user do not

### Requirement: Tokens stay on the server
No BFF response, cookie or log line SHALL contain an access, refresh or ID
token, an authorization code, a client secret or a session cookie value.

#### Scenario: Request logs
- **WHEN** the identity provider redirects to `/bff/callback?code=…&state=…`
- **THEN** the request log records the path `/bff/callback` without the query
