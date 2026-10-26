## ADDED Requirements

### Requirement: Instance apps exist only through the server CLI
An instance app is a confidential sign-in client of the whole instance (no
workspace), for the operator's own other apps. It SHALL be registered,
listed, given a new secret and removed only by `campbooks-server admin apps
add|list|rotate-secret|remove`, on the migration login; no HTTP endpoint
SHALL do any of these, and the runtime role SHALL be refused any insert,
update or delete of an instance app except recording when it last got a
token. Registering SHALL print its client id (`app_` followed by 16 random
bytes in unpadded base64url) and a 256-bit random secret, shown this once
and stored as its SHA-256 only; rotating SHALL print a new secret once and
stop the old one at once; removing SHALL revoke every consent people gave
it and every refresh token it holds. Registering SHALL refuse: a name that
is empty, longer than 100 characters, reads as Campbooks or Not A Camp, or
mixes scripts in a word; 0 or more than 10 redirect URIs, or more than 10
post-logout redirect URIs; a URI that is not `https` with a host (loopback
`http` only when the instance runs in development), has a fragment or
userinfo, is longer than 2048 characters, is not in its canonical form, or
is given twice; a scope ceiling that is empty or holds anything but
`openid`, `profile`, `email`, `offline_access` and the API scopes a third
party may hold. Each change SHALL be recorded in the identity audit log
(`instance_app_registered`, `instance_app_secret_rotated`,
`instance_app_removed`), with ids and counts only.

#### Scenario: Register, list, rotate, remove
- **WHEN** an operator registers Applicamp, lists the instance apps, rotates its secret and removes it
- **THEN** the registration prints its client id and secret, the list shows its name, scopes and redirect URIs and never a secret, the old secret answers 401 `invalid_client` after the rotation while the new one works, and after the removal the app is unknown to the token endpoint

#### Scenario: The secret is kept hashed
- **WHEN** an instance app is registered
- **THEN** the stored `secret_hash` is the secret's SHA-256 and the secret itself is stored nowhere

#### Scenario: A ceiling above what a third party may hold
- **WHEN** an operator registers an app with `identity:read`, `workspaces:write`, `admin:access` or an unknown scope in its ceiling
- **THEN** the command refuses it and nothing is stored

#### Scenario: A plain http redirect
- **WHEN** an operator registers an app with `http://applicamp.example.com/cb`, or with `http://127.0.0.1:4000/cb` on an instance not in development
- **THEN** the command refuses it

#### Scenario: The runtime role cannot change one
- **WHEN** a statement on the runtime role inserts an instance app, widens one's ceiling, adds a redirect URI, sets its secret, changes its kind or deletes it
- **THEN** the database refuses it, while recording its last use succeeds

### Requirement: Instance apps sign people in
An instance app SHALL use the authorization code with PKCE (S256, required
as for every client) and authenticate at the token endpoint with its secret
(`client_secret_basic` or `client_secret_post`); without it or with another
the answer SHALL be 401 `invalid_client`. Its redirect URIs SHALL match
exactly, with no loopback port leeway. An authorization SHALL be granted the
scopes it asks for that are in the app's ceiling and that a third party may
hold, and no other; when none is left the answer SHALL be `invalid_scope`.
Its tokens SHALL be workspace-bound and stand on a consent like any other
third-party app's: the consent screen once per person for the exact `https`
redirect (remembered as for any client), the workspace the person picks,
listed in Connected apps and revocable there. Its ID token SHALL carry
`sub`, `email`, `email_verified`, `name` (with `profile` and `email`) and
the request's `nonce`; with `offline_access` it SHALL get a refresh token,
rotated on every use with reuse detection. The consent screen SHALL show it
with kind `instance`, as added by this server, not as unverified, and not
as Campbooks' own (`verified` false).

#### Scenario: Code, PKCE and the secret
- **WHEN** a person allows Applicamp in Home and Applicamp redeems the code with its PKCE verifier and its secret by HTTP Basic
- **THEN** it gets an access token bound to Home, a refresh token and an ID token with the person's `sub`, `email`, `email_verified`, `name` and the `nonce`

#### Scenario: Without the verifier or the secret
- **WHEN** Applicamp redeems a code without its PKCE verifier, or without its secret
- **THEN** the answers are `invalid_request` and 401 `invalid_client`, and the code still works afterwards with both

#### Scenario: A redirect URI it did not register
- **WHEN** an authorization names Applicamp's callback with a trailing slash, a query, another port, another host, or a loopback URI
- **THEN** the answer is 400 `invalid_redirect_uri` and nothing is redirected

#### Scenario: Asking above the ceiling
- **WHEN** Applicamp, whose ceiling lacks `system:read`, asks for `openid … system:read`
- **THEN** the consent screen and the token list every scope asked for but `system:read`; asking only for scopes above the ceiling answers `invalid_scope`

#### Scenario: Asked once, then remembered
- **WHEN** a person allows Applicamp in Work and Applicamp asks again for the same scopes
- **THEN** the second authorization returns a code at once, bound to Work

#### Scenario: Revoked, then asked again
- **WHEN** the person revokes Applicamp in Connected apps
- **THEN** its access token answers 401, its refresh token `invalid_grant`, and its next authorization shows the consent screen

#### Scenario: A removed app's tokens
- **WHEN** an operator removes Applicamp
- **THEN** its refresh tokens answer 401 `invalid_client` at the token endpoint and its access tokens answer 401 at once

## MODIFIED Requirements

### Requirement: Revocation and logout
`/oauth2/revoke` SHALL revoke the sign-in a refresh token belongs to, for the
client that holds it, and SHALL answer 200 for unknown tokens (RFC 7009).
`/oauth2/end_session` (GET or form POST) SHALL require an `id_token_hint`
this issuer signed for one of Campbooks' own clients or an instance app
(expired is fine); without one it SHALL answer 400 `invalid_id_token_hint`
and end nothing. With one it SHALL end the issuer session the hint names and
that sign-in's refresh tokens, and redirect only to a
`post_logout_redirect_uri` the hint's client registered, matched exactly,
with `state` appended.

#### Scenario: Revoked refresh token
- **WHEN** a client revokes its refresh token and then uses it
- **THEN** the refresh fails with `invalid_grant`

#### Scenario: Cross-site sign-out refused
- **WHEN** another site sends a browser to `/oauth2/end_session` without a valid `id_token_hint`
- **THEN** the response is 400 with `code` = `invalid_id_token_hint` and the person's tokens keep working

#### Scenario: An instance app signs out
- **WHEN** Applicamp sends the browser to `/oauth2/end_session` with its ID token as the hint and its registered post-logout URI
- **THEN** the browser goes back to that URI with `state`, the sign-in's refresh tokens stop working, and another post-logout URI ends the session without a redirect
