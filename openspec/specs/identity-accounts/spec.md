# identity-accounts Specification

## Purpose
Who a person is to Campbooks: an account with one or more ways in, linked
only by proven ownership, where the provider account a person signs up with
is also their first mailbox (ADR 0014).

## Requirements

### Requirement: Accounts and provider ways in
An account SHALL have one or more ways in. A provider identity SHALL be keyed
by provider, issuer and subject, and SHALL belong to at most one account. The
account id SHALL be the `sub` of every token issued for the person.

#### Scenario: The same identity signs in to the same account
- **WHEN** a person who signed up with a Google account signs in with it again in another browser
- **THEN** no account is created and the new tokens carry the same `sub`

### Requirement: Linking by proven ownership only
A provider identity SHALL be linked to an account only by the provider round
trip that created the account. The server SHALL NOT link, suggest linking or
reveal an existing account on the basis of an email address, verified or
not.

#### Scenario: Same email, different provider account
- **WHEN** an account exists for Google account A with address ana@example.com and a person signs in with Google account B that reports the same address
- **THEN** the person is offered a new account, and creating it yields a different `sub`

### Requirement: Signing in with Google
Signing in with Google SHALL use the authorization code flow with a `state`
bound to the browser that started it by a cookie, a `nonce`, and PKCE S256. A
round trip SHALL be usable once, only from that browser, and only within ten
minutes; otherwise the browser SHALL return to the sign-in screen with
`error=invalid_state`. The server SHALL validate Google's ID token
signature against Google's published keys, plus its issuer, audience,
`azp`, expiry and nonce. A cancelled consent SHALL return to the sign-in
screen with `error=cancelled_at_provider`. A known identity SHALL start a
session for its account. An unknown identity SHALL lead to the new-account
screen, and no account SHALL exist until the person confirms it.

#### Scenario: Replayed or foreign callback
- **WHEN** a Google callback is opened a second time, or by a browser that did not start the round trip
- **THEN** the browser is sent to `/sign-in?error=invalid_state` and nobody is signed in

#### Scenario: Unknown identity
- **WHEN** a Google account no Campbooks account uses completes the round trip
- **THEN** the browser is sent to `/sign-in/new`, which shows the provider and address, and no account exists until `POST /auth/new-account`

### Requirement: Creating an account needs the browser and the screen
`POST /auth/new-account` SHALL require the `X-Campbooks-CSRF: 1` header from
the same origin and the pending sign-up cookie of the browser that completed
the round trip; otherwise it SHALL answer 403 `csrf_check_failed` or 404.

#### Scenario: Forged request
- **WHEN** a cross-site form posts to `/auth/new-account`
- **THEN** the response is 403 with `code` = `csrf_check_failed`

### Requirement: The first mailbox comes with sign-up
A Google round trip SHALL ask in one consent for `openid email profile`, the
mailbox scopes the grant-intake port reports for Google, and offline access.
When a new account is created from a round trip that granted every mailbox
scope and a refresh token, the grant SHALL be held, sealed, for that account
for seven days. When the person adopts it into a workspace they belong to,
it SHALL be handed to the grant-intake port and then forgotten. A grant not
adopted in time SHALL be deleted. A sign-in to an existing account SHALL NOT
store or hand over a grant. When Google returns no refresh token for a new
account whose mailbox was granted, the server SHALL ask Google once more
with `prompt=consent` for that account.

#### Scenario: Sign up, then first workspace
- **WHEN** a person signs up with Google granting mail access, creates a workspace and calls `identity.adopt_held_grant` for it
- **THEN** the grant-intake port receives the grant (with its refresh token) for that workspace and person, and the held grant is gone

#### Scenario: Mail permission declined
- **WHEN** the person unchecks the mail permission on Google's consent screen
- **THEN** the account is created, nothing is held, and sign-in succeeds

#### Scenario: Consented before
- **WHEN** Google returns no refresh token for a new account whose mailbox was granted
- **THEN** the browser is sent back to Google once with `prompt=consent` and `login_hint` naming that account

### Requirement: Identity audit trail without personal data
Account creation, sign-ins and refresh-token reuse SHALL be recorded as audit
events and published as `identity.account_created`, `identity.signed_in` and
`identity.refresh_reuse_detected`, holding ids, provider names and methods
only. No log line SHALL contain a code, token, cookie, client secret or email
address.

#### Scenario: A sign-in leaves no secret in the logs
- **WHEN** a person signs up through the web app and the app redeems the code, with every log level captured
- **THEN** no captured line contains either authorization code, a cookie value, the provider client secret or the person's address
