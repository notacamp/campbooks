# web-bff Specification

## Purpose
The web backend-for-frontend: how the browser app signs in through the
identity provider, keeps a server-side session behind an httpOnly cookie, and
reaches the API without ever holding a token.

## Requirements

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
