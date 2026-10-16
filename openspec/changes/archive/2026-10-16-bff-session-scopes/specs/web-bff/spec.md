## ADDED Requirements

### Requirement: Sessions keep up with the scopes asked for
Each web session SHALL record the scopes it asked for at sign-in. When the
BFF asks for a scope a session did not ask for (a release added it), or the
session recorded none, the session SHALL end when it is next used, as an
expired session does: it is deleted, its refresh token is revoked, the
identity provider's own session is left as it is, and the BFF answers as for
no session (401 `session_required` on the API, `authenticated: false` on the
current session). The comparison SHALL be between the scopes asked for, in any
order, never the scopes granted, so a sign-in can never end its own session.

#### Scenario: A release adds a scope
- **WHEN** a person signed in before the BFF asked for `assistant:read`, and their app calls the API
- **THEN** the session ends, its refresh token is revoked, the BFF answers 401 `session_required`, and the app signs in again and comes back with every scope

#### Scenario: Right after signing in
- **WHEN** a person has just signed in
- **THEN** their session asked for every scope the BFF asks for, and it is never ended for its scopes
