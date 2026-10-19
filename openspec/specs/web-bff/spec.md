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
SHALL end any session the browser already had, revoking its refresh token at
the identity provider and leaving the identity provider's session as it is,
start a new session, set the session cookie, clear the login cookie and answer
200 with a same-origin page that moves on to `return_to`, never with a
redirect.

#### Scenario: Successful sign-in
- **WHEN** the identity provider redirects back with a valid code and the state of a sign-in started by this browser
- **THEN** the response is 200 HTML that navigates to `return_to`, sets the session cookie and stores no token in it

#### Scenario: Signing in again in the same browser
- **WHEN** a browser that holds a session signs in again
- **THEN** its earlier session ends and that session's refresh token is revoked, and the new session works

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
as a hash (as are the tokens it replaced), and the access, refresh and ID
tokens encrypted with a key derived from the instance secret key, each bound
to its session and column. A started sign-in SHALL store its state and its
login cookie only as hashes, and its PKCE verifier and nonce encrypted the
same way, bound to the sign-in; nothing a request carried SHALL be stored in
plain text. The BFF SHALL reach these tables only through its functions
(see `access`). A session SHALL end when unused for the idle
timeout, at the absolute timeout, on logout or on a back-channel logout. The
absolute timeout (`CAMPBOOKS_BFF__ABSOLUTE_TIMEOUT_HOURS`) SHALL be between 1
and 8 784 hours (366 days); the server SHALL refuse to start otherwise,
naming the setting. Its
cookie value SHALL be replaced periodically, the previous value staying valid
for a short grace period. Ended sessions SHALL be deleted by a scheduled job.
A session whose tokens can no longer be decrypted SHALL end. A use SHALL be
recorded only with an idle deadline after it: when the server's clock has
passed the session's absolute end while the database's (behind it) still
finds the session, the use is not recorded and the request goes on.

#### Scenario: Idle session
- **WHEN** a session has not been used for longer than the idle timeout
- **THEN** `/bff/session` answers `{ "authenticated": false }` and clears the cookie

#### Scenario: Rotation
- **WHEN** a session's cookie value is older than the rotation interval and the browser makes a request
- **THEN** the response sets a new cookie value, and requests still carrying the old value succeed during the grace period only

#### Scenario: Pruning
- **WHEN** the prune job runs
- **THEN** sessions past their idle or absolute deadline and expired sign-ins are deleted, by the database's clock and in bounded batches, and live ones are kept

#### Scenario: Stored at rest
- **WHEN** a person has signed in, the cookie has rotated twice, and another sign-in is in flight
- **THEN** the current, previous and retired cookies, the state and the login cookie are stored only as their SHA-256, the tokens, the verifier and the nonce are encrypted and open only for their own row, and no cookie, state, nonce or verifier appears in any row as text or bytes

#### Scenario: An absolute timeout out of bounds
- **WHEN** the server starts with `CAMPBOOKS_BFF__ABSOLUTE_TIMEOUT_HOURS` of 0 or 8 785
- **THEN** it refuses to start, naming the setting, and starts with 8 784

#### Scenario: The server's clock ahead of the database's
- **WHEN** a session is found by the database's clock but its absolute end has passed by the server's
- **THEN** the request is answered as usual, not as a server error, and the session ends by either clock soon after

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
cause one refresh. A refused refresh SHALL end the session. When the API
refuses the session's access token as invalid (401 `invalid_token`: its
sign-in ended, though the token has not expired), the BFF SHALL force one
refresh for that request, unless another request already replaced the token.
A refused refresh ends the session and the BFF answers 401
`session_required`. When the identity provider cannot be reached, the BFF
SHALL answer 503 `idp_unavailable` and keep the session, so a later request
refreshes. A refresh that works SHALL send the request again, once, with the
fresh token, when its body is empty or of a known length of at most 1 MiB,
held by the BFF. The API refuses a token while authenticating, before any
handler runs, and the copy carries the same `Idempotency-Key`. A larger or
streamed body SHALL get the API's refusal back, never sent twice, with the
fresh token held for the next request. One request SHALL cause at most one
forced refresh. The same applies to `/bff/admin`, though the admin API
answers 404, never `invalid_token`, to a token it cannot accept.

#### Scenario: Token attached
- **WHEN** a signed-in browser requests `/bff/api/v1/<route>` of a scoped endpoint
- **THEN** the API sees the signed-in user as the principal and the browser never sees the token

#### Scenario: Refresh once under concurrency
- **WHEN** several requests of one session arrive while its access token is about to expire
- **THEN** exactly one refresh is made, all requests succeed, and the rotated refresh token is stored

#### Scenario: No session
- **WHEN** a browser without a live session requests `/bff/api/v1/system/version`
- **THEN** the response is 401 with `code` = `session_required`

#### Scenario: A refused token whose sign-in ended
- **WHEN** the API refuses a session's access token as `invalid_token` because the way in the session used was removed, or the person signed out everywhere on another device, and the refresh is refused too
- **THEN** that request gets 401 `session_required`, the session is gone and `/bff/session` says signed out, after one refresh attempt

#### Scenario: A refused token refreshed
- **WHEN** the API refuses a session's access token as `invalid_token` and the refresh works
- **THEN** a `GET`, and a `POST` with a small body, are sent again once with the fresh token and succeed with the same body, while a `POST` of more than 1 MiB gets the 401 back and the app's retry succeeds with the held fresh token

#### Scenario: Two requests refused together
- **WHEN** two requests of one session are refused with the same token at once
- **THEN** one refresh is made, the second request uses the token it stored, and both succeed

#### Scenario: A refused token with the identity provider down
- **WHEN** the API refuses a session's access token and the identity provider's token endpoint answers 503
- **THEN** the response is 503 with `code` = `idp_unavailable`, the session stays signed in, and once the identity provider is back the next request succeeds

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

### Requirement: Sessions keep up with the scopes asked for
Each web session SHALL record the scopes its sign-in asked for, as recorded
when that sign-in started. When the BFF asks for a scope a session did not ask
for (a release added it), or the session recorded none, the session SHALL end
when it is next used: it is deleted, its refresh token is revoked, the
identity provider's own session is left as it is (so a new sign-in can go
straight through), its cookie is cleared, and the BFF answers as for no
session (401 `session_required` on the API, `authenticated: false` on the
current session). Signing out SHALL end such a session in full, the identity
provider's session included, like any other. The comparison SHALL be between
the scopes asked for, in any order, never the scopes granted, so a sign-in can
never end its own session. Asking for fewer scopes SHALL end no session.

#### Scenario: A release adds a scope
- **WHEN** a person signed in before the BFF asked for `assistant:read`, and their app calls the API
- **THEN** the session ends, its refresh token is revoked, the BFF answers 401 `session_required`, and the app asks them to sign in again

#### Scenario: Right after signing in
- **WHEN** a person has just signed in
- **THEN** their session asked for every scope the BFF asks for, and it is never ended for its scopes

#### Scenario: A sign-in across a deploy
- **WHEN** a sign-in started before a release added a scope and finished after it
- **THEN** the session records the scopes it asked for, and ends on its first use

#### Scenario: Signing out of an outdated session
- **WHEN** a person whose session predates a new scope signs out
- **THEN** the session and the identity provider's session both end

### Requirement: The instance secret key can be rotated
The server SHALL accept a previous instance secret key beside the current one
(`CAMPBOOKS_SECRET_KEY_PREVIOUS_FILE` or `CAMPBOOKS_SECRET_KEY_PREVIOUS`).
Everything SHALL be sealed, signed and derived with the current key. The
previous key SHALL only open and verify what was sealed or signed before:
- web sessions' and sign-ins' sealed values;
- connected accounts' credentials;
- the sign-in signing keys and held grants;
- signed and sealed opaque values (cursors, resume tokens);
- the binding tags of sends and changes at mail providers.

What the previous key opened SHALL be sealed again with the current key as it
is used:
- the signing keys when they are loaded;
- a connection's access token when it is used, and its refresh token at its
  next refresh;
- a web session's refresh and ID tokens at its next refresh.

What the previous key signed SHALL be signed again with the current key in
the background. While a previous key is set, a job of each module SHALL run in every
workspace with a change feed, at most every 15 minutes. It SHALL re-tag
every taught rule (at its generation), rule application, running rule run,
change request, scheduled or stored send and pending outbound change (a
change waiting to be made at a mail provider) whose MAC verifies under the
previous key only. The first run SHALL be at start-up. Each update SHALL be a compare-and-set on the old MAC, so
a row signed again meanwhile is left as it is, and a row that verifies under
neither key SHALL NOT be re-tagged. Each run SHALL log, per workspace,
either that nothing is bound under the previous key any more or how many
rows are left for the next run.

The server SHALL refuse to start, naming the variable, when a previous key is
set without a current one, is the same key, or is not 32 bytes of base64. No
key SHALL appear in a log, an error or a `Debug` output. Without the previous
key, what only it sealed SHALL fail closed, as for any wrong key.

#### Scenario: Old values open and are sealed again
- **WHEN** the key is rotated with the old one as the previous key, and a connection's access token is used, its token is refreshed, and the signing keys are loaded
- **THEN** each opens, and is stored sealed with the new key, which alone opens it afterwards

#### Scenario: The previous key removed
- **WHEN** the previous key is removed after everything was sealed again
- **THEN** the new key alone opens everything, and the old key alone opens nothing

#### Scenario: Rules and their changes signed again
- **WHEN** a rule moved mail under the old key, the key is rotated, the re-tag runs and the previous key is removed
- **THEN** the rule still acts on new mail, its changes are still put back, and a second re-tag finds nothing to do

#### Scenario: A wrong key
- **WHEN** neither the current nor the previous key sealed a value
- **THEN** it does not open: a connection needs connecting again, and the server refuses to start on signing keys it cannot open

#### Scenario: A misconfigured previous key
- **WHEN** `CAMPBOOKS_SECRET_KEY_PREVIOUS` is set without `CAMPBOOKS_SECRET_KEY`, equal to it, or not valid base64
- **THEN** the server refuses to start with an error naming the variable and showing no key

#### Scenario: Each kind of signed row
- **WHEN** a running rule run, a change request, a scheduled send, a stored send and a pending outbound change are each signed under the old key, the key is rotated, the re-tag runs and the previous key is removed
- **THEN** the run works through its mail, the change request stands, the scheduled send is released, the stored send stands and the outbound change is made, each under the new key alone

### Requirement: Streamed answers pass through the BFF
An API answer that streams (`text/event-stream`: the change feed, a
streamed reply) SHALL reach the browser through `/bff/api/<path>` as it is
written, never buffered whole by the BFF, for safe and unsafe methods
alike. An unsafe request SHALL still need the CSRF header and a same-origin
request before it reaches the API, and the request's time limit, including
any limit the operation raised for itself, SHALL apply to the in-process
request as to a direct one.

#### Scenario: A streamed reply through the BFF
- **WHEN** a signed-in browser POSTs a streamed reply to `/bff/api/v1/w/{workspace_id}/conversations/{conversation_id}/reply/draft/stream` with the CSRF header
- **THEN** the response is `text/event-stream` and its events arrive as the API writes them

#### Scenario: Without the CSRF header
- **WHEN** the same POST arrives without `X-Campbooks-CSRF`
- **THEN** the BFF refuses it before the API runs
