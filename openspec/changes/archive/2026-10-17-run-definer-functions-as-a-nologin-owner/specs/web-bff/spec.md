## MODIFIED Requirements

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
A session whose tokens can no longer be decrypted SHALL end.

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
