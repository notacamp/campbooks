## Purpose

Who a person is to Campbooks: an account with one or more ways in (provider
identities and passkeys), linked only by proven ownership, where the
provider account a person signs up with is also their first mailbox.

## ADDED Requirements

### Requirement: Accounts and ways in
An account SHALL have one or more ways in: provider identities, each keyed by
provider, issuer and subject, and passkeys. A provider identity SHALL belong
to at most one account. The account id SHALL be the `sub` of every token
issued for the person. An account SHALL never be left without a way in: a
request to remove its last one SHALL be refused with `code` = `last_way_in`.

#### Scenario: Last way in
- **WHEN** a person whose only way in is a Google identity asks to remove it
- **THEN** the response is 409 with `code` = `last_way_in` and the identity is kept

#### Scenario: Several ways in reach one account
- **WHEN** a person has linked a Google and a Zoho identity and signs in with either
- **THEN** both sign-ins yield tokens with the same `sub`

### Requirement: Linking by proven ownership only
A provider identity SHALL be linked to an account only by the provider round
trip that created the account, or by a round trip started and completed
while signed in to that account. The server SHALL NOT link, suggest linking
or reveal an existing account on the basis of an email address, verified or
not. A round trip that returns an identity already linked to another account
SHALL be refused with `code` = `way_in_belongs_to_another_account`, and its
provider tokens SHALL be revoked.

#### Scenario: Same email, different account
- **WHEN** an account was created with Google identity A for ana@example.com and a signed-out person signs in with a Microsoft identity whose email is also ana@example.com
- **THEN** no link is made and the person is shown the new-account page, which does not mention any existing account

#### Scenario: Identity held by another account
- **WHEN** a person signed in to account X completes a link round trip that returns an identity linked to account Y
- **THEN** the response carries `code` = `way_in_belongs_to_another_account` and neither account changes

### Requirement: Provider sign-in
Sign-in with a configured provider SHALL use the authorization code flow
with a `state` bound to the browser, a `nonce`, and PKCE S256. It SHALL
validate the provider's ID token signature against the provider's published
keys, plus its issuer, audience, `azp`, expiry and nonce. For Microsoft the
issuer SHALL equal the tenant-specific issuer of the token's own `tid`. For
Zoho the accounts server SHALL be one of the known Zoho data-centre servers,
and any other value SHALL be refused before any request is made. A
known identity SHALL start a session for its account. An unknown identity
SHALL lead to a new-account confirmation before any account is created.

#### Scenario: Forged Zoho accounts server
- **WHEN** a Zoho callback names an `accounts-server` that is not a Zoho data centre
- **THEN** the sign-in fails with `code` = `login_failed` and no request is sent to that host

#### Scenario: Replayed callback
- **WHEN** the same provider callback URL is opened twice, or by another browser
- **THEN** the second attempt fails with `code` = `invalid_state`

### Requirement: The first mailbox comes with sign-up
A sign-in round trip SHALL ask the provider, in the same consent, for the
identity scopes, the mailbox scopes the instance's connectors can use for
that provider, and offline access. When a new account is created from a
round trip that granted mailbox scopes and a refresh token, the grant SHALL
be held sealed for that account for at most 7 days. When the person adopts
it into a workspace, it SHALL be handed to the connectors through the grant
intake port. If it is not adopted in time, it SHALL be revoked at the
provider and deleted. A sign-in to an existing account SHALL NOT change any
existing mailbox connection.

#### Scenario: Sign up, then first workspace
- **WHEN** a person signs up with Google granting Gmail access, creates their first workspace and adopts the held grant
- **THEN** the workspace has a mail connection for that Google account granted by that person, and the held grant is gone

#### Scenario: Grant never adopted
- **WHEN** 7 days pass after sign-up without the held grant being adopted
- **THEN** the grant is revoked at the provider and deleted

#### Scenario: Mail scope declined
- **WHEN** the person unchecks the mail permission on the provider's consent screen
- **THEN** the account is created, no grant is held, and sign-in succeeds

### Requirement: Connecting a mailbox adds a way in
Connecting or reconnecting a provider mailbox while signed in SHALL run
through the identity round trip, link the provider identity to the signed-in
account (or refuse as above), and hand the grant to the connectors for the
current workspace. Disconnecting a mailbox SHALL NOT remove the way in, and
removing a way in SHALL NOT disconnect a mailbox.

#### Scenario: Second mailbox becomes a way in
- **WHEN** a signed-in person connects a Zoho mailbox to their workspace, signs out and signs in with that Zoho account
- **THEN** they are signed in to the same account

### Requirement: Step-up for sensitive changes
Adding or removing a way in, registering a passkey, and creating or rotating
a client secret SHALL require a sign-in within the last 10 minutes; an older
session SHALL get 401 with `code` = `reauthentication_required`.

#### Scenario: Stale session removes a way in
- **WHEN** a person who signed in an hour ago asks to remove a way in
- **THEN** the response is 401 with `code` = `reauthentication_required` and nothing is removed

### Requirement: Identity audit trail without personal data
Account creation, ways in added or removed, sign-ins, refresh-token reuse,
client registration and consent changes SHALL be recorded as audit events
and published as `identity.*` domain events. Neither SHALL contain email
addresses, tokens, codes or secrets. No log line, span field or metric
attribute SHALL contain a token, code, PKCE verifier, state, nonce, client
secret, cookie, WebAuthn assertion or email address.

#### Scenario: Telemetry stays clean
- **WHEN** a full sign-up, sign-in and token refresh run with telemetry captured
- **THEN** no captured field contains any of the values exchanged
