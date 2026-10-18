## MODIFIED Requirements

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
