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
- **THEN** sessions past their idle or absolute deadline and expired sign-ins are deleted, by the database's clock and in bounded batches, and live ones are kept

#### Scenario: Stored at rest
- **WHEN** a person has signed in, the cookie has rotated twice, and another sign-in is in flight
- **THEN** the current, previous and retired cookies, the state and the login cookie are stored only as their SHA-256, the tokens, the verifier and the nonce are encrypted and open only for their own row, and no cookie, state, nonce or verifier appears in any row as text or bytes
