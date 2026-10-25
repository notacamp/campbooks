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
trip that created the account, or by one started with `identity.start_link`
and completed while signed in to that account. The server SHALL NOT link,
suggest linking or reveal an existing account on the basis of an email
address, verified or not. A provider identity that is another account's way
in SHALL NOT be linked, merged or moved: the round trip SHALL end with
`link_error=way_in_belongs_to_another_account`, no connection SHALL be made,
and the provider's tokens SHALL be dropped without being revoked.

#### Scenario: Same email, different provider account
- **WHEN** an account exists for Google account A with address ana@example.com and a person signs in with Google account B that reports the same address
- **THEN** the person is offered a new account, and creating it yields a different `sub`

#### Scenario: Linking another account's way in
- **WHEN** a signed-in person links a Google account that is already a way into another Campbooks account
- **THEN** the browser returns with `link_error=way_in_belongs_to_another_account`, both accounts keep exactly the ways in they had, and no revocation request reaches Google

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

### Requirement: New accounts follow the signup policy
On an `invite_only` instance, an unknown provider identity SHALL become an
account only when the provider verified its address and that address has a
pending invitation; otherwise the round trip SHALL end at the sign-in screen
with `error=signup_invite_only`, and no account, pending sign-up, tokens or
held grant SHALL be kept. The policy SHALL be checked again when the account
is created. `open`, `code` and `approval` instances SHALL let new accounts
in; their gate is creating a workspace.

#### Scenario: Invite only, not invited
- **WHEN** an `invite_only` instance has no pending invitation for the Google account's verified address
- **THEN** the browser lands on `/sign-in?error=signup_invite_only` and nothing about that person is stored

#### Scenario: Invite only, invited
- **WHEN** the verified address has a pending invitation
- **THEN** the new-account step is offered and creating the account succeeds

#### Scenario: Open, code or approval
- **WHEN** the instance's policy is `open`, `code` or `approval`
- **THEN** a new Google account can create a Campbooks account

### Requirement: The first mailbox comes with sign-up
A Google round trip SHALL ask in one consent for `openid email profile`, the
mailbox scopes the grant-intake port reports for Google, and offline access.
When a new account is created from a round trip that granted every mailbox
scope and a refresh token, the grant SHALL be held, sealed, for that account
for seven days. When the person adopts it into a workspace they belong to,
it SHALL be handed to the grant-intake port and then forgotten; adopting
needs both `identity:write` and `connectors:write`. A grant not
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

### Requirement: The redirect path sent to a provider is configurable
The `redirect_uri` Campbooks sends to a sign-in provider, in the authorization
request and again in the code exchange, SHALL be the instance's public URL plus
the provider's `CAMPBOOKS_<PROVIDER>__REDIRECT_PATH`, defaulting to
`/auth/callback/<provider>`. The path SHALL be absolute: it starts with a
single `/` and has no scheme, host, query, fragment or dot segments. A
configured path that is not SHALL stop the server at startup with an error
naming the setting. The route Campbooks serves SHALL stay
`/auth/callback/{provider}` whatever the setting says.

#### Scenario: Default
- **WHEN** `CAMPBOOKS_GOOGLE__REDIRECT_PATH` is unset or blank
- **THEN** the authorization request carries `redirect_uri=<public url>/auth/callback/google`

#### Scenario: Pre-registered path behind a proxy
- **WHEN** `CAMPBOOKS_GOOGLE__REDIRECT_PATH=/oauth/gmail/callback` and a proxy rewrites that path to `/auth/callback/google`
- **THEN** the authorization request and the token request both carry `redirect_uri=<public url>/oauth/gmail/callback`, and the sign-in completes

#### Scenario: Invalid path
- **WHEN** the setting is `https://evil.example.com/cb`, `//evil.example.com/cb`, `cb`, `/cb?x=1`, `/cb#f` or `/a/../b`
- **THEN** the server refuses to start with an error naming `CAMPBOOKS_GOOGLE__REDIRECT_PATH`

### Requirement: Adding a way in while signed in
`identity.start_link` (`identity:write`) with purpose `link` SHALL need a
configured provider and a sign-in within the last ten minutes, and SHALL
return a URL of `/auth/start/{provider}?link=<ticket>`. The ticket SHALL be
single use, expire after two minutes, and start a round trip only in a
browser whose issuer session belongs to the same account; otherwise nothing
SHALL start and the browser SHALL return with `link_error=sign_in_again` or
`link_error=link_expired`. The round trip SHALL ask for consent again
(`prompt=consent`), and its callback SHALL find the same browser still
signed in to that account, or link nothing and return with
`link_error=sign_in_again`. Completed, it SHALL link the provider identity to the
account (or refresh it when already linked), audit it, publish
`identity.way_in_added`, and return the browser to `return_to?linked=<provider>`.

#### Scenario: Add Zoho as a way in
- **WHEN** a person who signed up with Google, signed in five minutes ago, starts a `link` to Zoho and approves it at Zoho
- **THEN** the account has two ways in, `identity.way_in_added` is published, and signing in with that Zoho account later opens the same account

#### Scenario: Stale sign-in
- **WHEN** the person last signed in an hour ago and starts a `link`
- **THEN** the response is 403 with `code` = `step_up_required` and no ticket exists

#### Scenario: A ticket opened in another browser
- **WHEN** a link ticket is opened by a browser signed in to a different account, or to none
- **THEN** no round trip starts, the ticket is spent, and the account's ways in are unchanged

### Requirement: Connecting and reconnecting a mailbox through identity
`identity.start_link` with purpose `connect` SHALL need a workspace the caller
belongs to (404 otherwise), a provider whose mailbox the instance can sync
(409 `mailbox_unsupported` otherwise) and a recent sign-in. With purpose
`reconnect` it SHALL need a connection the caller granted in that workspace
(404 otherwise), and a recent sign-in unless that connection needs
re-authorisation. Both SHALL ask for identity, mailbox and offline access with
`prompt=consent`. A connect SHALL link the identity as for `link`, unless it
was started with `sign_in: false` (a shared mailbox): then it SHALL add no
way in, still refuse an identity that is another account's way in, and hand
the grant over marked as one the person does not sign in with (unless the
identity already is one of their ways in). A
reconnect SHALL accept only the connection's own provider account, refusing
any other with `link_error=account_mismatch` before writing anything, and
SHALL never add a way in. Then the grant SHALL go to the grant-intake port
for that workspace (naming the connection for `reconnect`). A refusal of the
port SHALL return the browser with `link_error=<its code>`; success SHALL
return it with `connected=<connection id>`.

#### Scenario: Connect a second Gmail
- **WHEN** a signed-in member connects a second Google account in their workspace
- **THEN** that Google account is a way in, and the grant-intake port receives its grant with a refresh token for that workspace

#### Scenario: A connect that would add a way in, off the allow-list
- **WHEN** the sign-in allow-list does not let an account in and a member connects it with `sign_in: true`
- **THEN** the browser returns with `link_error=sign_in_not_allowed` before any way in or mailbox record is written or any grant handed over

#### Scenario: A shared mailbox off the allow-list
- **WHEN** a member connects that account with `sign_in: false`
- **THEN** it is connected, no way in or mailbox record is written, and signing in with it is still refused

#### Scenario: Connect a shared mailbox
- **WHEN** a member connects `support@example.com` with `sign_in: false`
- **THEN** the grant-intake port receives it with `signs_in` false, the member's ways in are unchanged, and no `identity.way_in_added` is published

#### Scenario: Reconnect after a revoked grant
- **WHEN** a member's connection needs re-authorisation and they reconnect it
- **THEN** the provider is asked for that account (`login_hint`), the port receives the grant naming that connection, and no recent sign-in was needed

#### Scenario: Someone else's connection
- **WHEN** a member starts a `reconnect` for a connection a colleague granted
- **THEN** the response is 404

#### Scenario: Another provider account comes back from a reconnect
- **WHEN** a reconnect is started for a connection and a different Google account consents at the provider
- **THEN** the browser returns with `link_error=account_mismatch`, and the account's ways in, sessions and connections are unchanged

#### Scenario: Renewing a grant that still works
- **WHEN** a member whose sign-in is an hour old starts a `reconnect` for a healthy connection
- **THEN** the response is 403 with `code` = `step_up_required`

### Requirement: Listing and removing ways in
`identity.list_ways_in` (`identity:read`) SHALL list the caller's provider
ways in (provider, address as given at link time, how and when linked, last
used, and, for a way in a connect created, how many workspaces its mailbox is
connected in and the connections among them that are known), the providers
configured on the instance, and, as
`mailbox_providers`, the configured providers a mailbox can be connected
from (`identity.start_link` with `connect` accepts them: the instance has a
mail adapter for them), in the same order.
`identity.remove_way_in` (`identity:write`) SHALL need a recent sign-in,
lock the account, and refuse with 409 `last_way_in` when the way in is the
account's only one, counting provider identities and passkeys. A way in of
another account SHALL be 404. Removing SHALL end every session signed in
with that way in and revoke their refresh tokens, SHALL NOT disconnect any
mailbox, SHALL be audited, and SHALL publish `identity.way_in_removed`.

#### Scenario: Remove the last way in
- **WHEN** a person with only a Google way in removes it
- **THEN** the response is 409 with `code` = `last_way_in` and the way in remains

#### Scenario: Whoever signed in with it is signed out
- **WHEN** a way in is removed while another browser is signed in with it
- **THEN** that browser's session ends and its refresh token is refused

#### Scenario: Remove one of two
- **WHEN** a person with a Google and a Zoho way in removes the Zoho one
- **THEN** it is gone, signing in with Zoho now leads to the new-account screen, and every mailbox connection is untouched

#### Scenario: Mailbox providers
- **WHEN** an instance configures Google, Microsoft and Zoho for sign-in and has mail adapters for Google and Zoho only
- **THEN** `providers` is `google`, `microsoft`, `zoho` and `mailbox_providers` is `google`, `zoho`

### Requirement: Signing in with Microsoft
Signing in with Microsoft SHALL use the tenant set by
`CAMPBOOKS_MICROSOFT__TENANT` (`common` by default), with the same state,
nonce, PKCE and browser binding as Google. The ID token SHALL be validated
against the common key set with its issuer required to be
`https://login.microsoftonline.com/{tid}/v2.0` for the token's own `tid`
(a GUID), and the identity SHALL be keyed by that issuer and `sub`. Its
address SHALL count as verified only when the token's `xms_edov` claim is
`true`.

#### Scenario: A token naming another tenant's issuer
- **WHEN** the ID token's `tid` is tenant A and its `iss` names tenant B
- **THEN** the browser lands on `/sign-in?error=login_failed` and nobody is signed in

#### Scenario: Unverified Microsoft address on an invite-only instance
- **WHEN** an invite-only instance has a pending invitation for the address a Microsoft account reports without `xms_edov`
- **THEN** the browser lands on `/sign-in?error=signup_invite_only`

### Requirement: Signing in with Zoho
Signing in with Zoho SHALL accept a callback's `accounts-server` only when it
is one of Zoho's accounts servers (`.com`, `.eu`, `.in`, `.com.au`, `.jp`,
`.ca`, `.sa`, `.uk`); anything else SHALL end at
`/sign-in?error=login_failed` before any request is made. The code SHALL be
exchanged at that server, and the ID token's issuer SHALL equal it.

#### Scenario: A made-up accounts server
- **WHEN** a Zoho callback names `accounts-server=https://evil.example.com`
- **THEN** no request is sent anywhere, and the browser lands on `/sign-in?error=login_failed`

#### Scenario: A European Zoho account
- **WHEN** a person in Zoho's EU data centre signs in
- **THEN** the code is exchanged at `https://accounts.zoho.eu`, and their identity's issuer is that server

### Requirement: Offline access only with a mailbox
A provider round trip SHALL ask for offline access only when it asks for
mailbox scopes. A sign-in with a provider that has no mailbox adapter SHALL
keep no provider token.

#### Scenario: Sign in with Zoho before its mail adapter
- **WHEN** a person signs up with Zoho on an instance with no Zoho mailbox adapter
- **THEN** the authorization request carries no `access_type=offline`, and no grant is held

### Requirement: Ways to sign in screen
Settings SHALL have a Ways to sign in page, made of section cards
(`settings-ui`) under the lead "How you get into Campbooks. Keep at least two,
so losing one never locks you out.": an "Accounts" card with a count listing
the person's provider ways in, with Remove behind an inline confirmation (and
`last_way_in` explained in place), Reconnect for this workspace's connections
of theirs that need re-authorisation, and, at its bottom, Add for each
configured provider; further sections other slices contribute (passkeys); a
"Sign in again" offer when an action needs a recent sign-in; a notice and a
"New" mark when a way in (other than the sign-up's) was added in the last
seven days; and Sign out everywhere as its own card, its button outlined in
the danger tone, behind an inline confirmation. The sign-in screen SHALL offer
every provider the server lists. Both SHALL be in en, pt-PT, es-ES and fr,
keyboard-operable with visible hints, and usable at 375px.

#### Scenario: Remove with confirmation
- **WHEN** the person presses Remove on a way in
- **THEN** an inline confirmation names the provider and address and says the mailbox stays connected, and nothing is removed until they confirm

#### Scenario: Microsoft and Zoho offered
- **WHEN** the server lists `google`, `microsoft` and `zoho` for a sign-in request
- **THEN** the sign-in screen shows the three, in that order, with `1`, `2` and `3`

#### Scenario: Added this week
- **WHEN** a Zoho account was added two days ago
- **THEN** its row is marked New and the page says to remove it and sign out everywhere if it wasn't the person

### Requirement: Zoho accounts server for the sign-in request
The authorization request to Zoho SHALL go to the accounts server named by `CAMPBOOKS_ZOHO__ACCOUNTS_SERVER`, default `https://accounts.zoho.com`. The setting SHALL be exactly one of Zoho's accounts servers; any other value (another host, `http`, a path) SHALL stop startup with an error naming the setting. The callback's `accounts-server` handling is unchanged.

#### Scenario: Default
- **WHEN** the setting is unset
- **THEN** the authorization URL starts with `https://accounts.zoho.com/oauth/v2/auth`

#### Scenario: A European app
- **WHEN** the setting is `https://accounts.zoho.eu`
- **THEN** the authorization URL starts with `https://accounts.zoho.eu/oauth/v2/auth`

#### Scenario: A value that is not a Zoho accounts server
- **WHEN** the setting is `https://accounts.example.com`, `http://accounts.zoho.eu` or `https://accounts.zoho.eu/oauth`
- **THEN** startup fails with an error naming `CAMPBOOKS_ZOHO__ACCOUNTS_SERVER`

### Requirement: Suspended accounts get in by no door
A suspended account SHALL be refused at every way into the issuer: a
provider sign-in SHALL send the browser to the sign-in screen with
`error=account_suspended` and start no session; a passkey sign-in SHALL
answer 403 `account_suspended` once the assertion verified (and
`passkey_not_accepted` before), starting no session; its issuer sessions SHALL no
longer sign it in to any client; its authorization codes and refresh tokens
SHALL be refused with `invalid_grant`; the access tokens already issued to it
SHALL be refused by the API at once (see "Revoked access tokens stop at
once"). Once unsuspended, the account SHALL sign in again with its ways in;
nothing ended by the suspension is restored, its old access tokens
included.

#### Scenario: Every door refuses
- **WHEN** an account that holds a refresh token, an unredeemed code and an issuer session is suspended
- **THEN** the refresh token and the code answer `invalid_grant`, `/oauth2/authorize` opens the sign-in screen instead of returning a code, and signing in with Google comes back with `error=account_suspended`

#### Scenario: Passkey of a suspended account
- **WHEN** a suspended account's passkey signs in with a valid assertion
- **THEN** the answer is 403 `account_suspended` and no session starts, while a tampered signature for the same passkey answers `passkey_not_accepted`

#### Scenario: Unsuspended
- **WHEN** the account is unsuspended and the person signs in with Google
- **THEN** the web client receives a code and redeems it for tokens

#### Scenario: Access tokens already issued
- **WHEN** an account is suspended while a client holds one of its access tokens, and is then unsuspended
- **THEN** the token gets 401 from the moment of the suspension and still after the unsuspension

### Requirement: Passkeys are ways in
A passkey SHALL be a discoverable WebAuthn credential registered with user
verification, belonging to one account. Its credential id SHALL be unique
across the instance. An account's ways in SHALL be its provider identities
and its passkeys, and the last one SHALL never be removed. The relying-party
id SHALL be the host of the public URL and the only accepted origin that
URL's origin; an instance whose public URL is plain http on a host other than
`localhost`, or an IP address, SHALL offer no passkeys (its passkey endpoints
answer 404).

#### Scenario: Passkeys on offer
- **WHEN** a sign-in request is read on an https instance with Google, Microsoft and Zoho configured
- **THEN** it lists `["google", "microsoft", "zoho", "passkey"]`

### Requirement: Adding a passkey while signed in
`identity.start_passkey_registration` (`identity:write`) SHALL need a
sign-in within the last five minutes (else 403 `fresh_sign_in_required`, and
the web app signs in again with `max_age=0`) and SHALL return creation
options: a single-use challenge valid five minutes and bound to the account,
the account id as user handle, `residentKey` and `userVerification`
required, attestation `none`, algorithms EdDSA (-8), ES256 (-7) and RS256
(-257), and the account's passkeys as `excludeCredentials`.
`identity.register_passkey` (`identity:write`, a sign-in within the last ten
minutes) SHALL take that challenge from the new credential's client data,
spending it whatever the outcome, verify the credential, store it with its
name (1 to 64 characters, default "Passkey"), audit it and publish
`identity.way_in_added` with method `passkey` and the passkey. A challenge
started by another account, spent or expired SHALL be 409
`passkey_expired`; a credential that does not verify SHALL be 403
`passkey_not_accepted`; a credential id already registered SHALL be 409
`passkey_already_registered`.

#### Scenario: Add a passkey
- **WHEN** a person who signed in two minutes ago adds a passkey and approves it on their device
- **THEN** it is listed among their passkeys and signing in with it later opens the same account

#### Scenario: Stale sign-in
- **WHEN** the person last signed in an hour ago and starts a registration
- **THEN** the response is 403 with `code` = `fresh_sign_in_required`

#### Scenario: Not fresh enough
- **WHEN** the person last signed in seven minutes ago and starts a registration
- **THEN** the response is 403 with `code` = `fresh_sign_in_required`, though removing a passkey still works

#### Scenario: Another account's challenge
- **WHEN** a credential made for one account's registration challenge is submitted by another account
- **THEN** the response is 409 with `code` = `passkey_expired` and nothing is stored

### Requirement: Passkey ceremonies store little and wait in small numbers
The ceremony endpoints SHALL read at most 16 KiB of body (else 400
`invalid_body`). A `request` they are given SHALL be at most 64 URL-safe
characters and name a waiting authorization request of the same browser, or
the answer SHALL be 404 and nothing stored. One client address SHALL start
at most ten ceremonies, and one account at most ten registrations, per five
minutes (a challenge's lifetime); one address SHALL create at most 5
passkey-only accounts an hour, and the instance at most 200; beyond that the
answer SHALL be 429 `rate_limited`. A browser SHALL keep one ceremony cookie
for all its waiting ceremonies, so a ceremony started later (the passkey
dialog after autofill) leaves earlier ones valid.

#### Scenario: An unknown or oversized request id
- **WHEN** sign-in options are asked for with a `request` that is unknown, expired, another browser's, longer than 64 characters or in a 20 KB body
- **THEN** the answer is 404 (400 for the body) and no challenge is stored

#### Scenario: Autofill, then the dialog
- **WHEN** the sign-in screen starts an autofill ceremony and the person then presses `P`
- **THEN** both challenges are valid in that browser, and the autofill pick still signs in

#### Scenario: Passkey sign-ups from one address
- **WHEN** one address creates a sixth passkey-only account within the hour
- **THEN** the answer is 429, while another address can still sign up

### Requirement: Signing in with a passkey
`POST /auth/passkey/sign-in/options` (CSRF header) SHALL return request
options for any of the site's passkeys (no `allowCredentials`, user
verification required) and set a `SameSite=Strict` ceremony cookie binding a
single-use, five-minute challenge to the browser. `POST /auth/passkey/sign-in`
SHALL take that challenge only for the browser holding the cookie, find the
passkey by credential id, require the user handle to be its account, verify
the assertion, and start an issuer session signed in with that passkey,
then answer where to go (the client's redirect URI with a code when an
authorization request waits). Both SHALL be rate-limited per client address.
A passkey no account has (removed, or never registered), a disabled
account's, or an assertion that does not verify SHALL be 403
`passkey_not_accepted`, and no session SHALL start. A missing, spent,
expired or foreign ceremony SHALL be 409 `passkey_expired`.

#### Scenario: Sign in from another device's browser
- **WHEN** a person with a synced passkey signs in with it on a new browser
- **THEN** the web client gets a code for the same account, and the ID token's `amr` is `["swk", "user"]`

#### Scenario: Replay
- **WHEN** an assertion that already signed someone in is posted again
- **THEN** the response is 409 with `code` = `passkey_expired`

#### Scenario: Relayed from another origin
- **WHEN** an assertion whose client data names another origin is posted with a valid ceremony
- **THEN** the response is 403 with `code` = `passkey_not_accepted` and no session starts

### Requirement: Sign-in allow-list
`CAMPBOOKS_SIGNUP__ALLOWED` SHALL be a comma-separated list where an entry
containing `@` is one exact address and any other entry is a domain matching
`*@domain` exactly (no subdomains), compared case-insensitively, entries
trimmed and empty ones ignored. Unset or empty SHALL mean no restriction. A
malformed entry SHALL stop startup with an error naming the setting and the
entry's position, never its text. When set, on top of the signup policy and
whatever invitations exist, the issuer SHALL refuse with `sign_in_not_allowed`
(and keep nothing) at: a provider sign-in whose provider-verified address is
not listed, for known and unknown accounts alike, before looking for an
account; creating an account from a pending sign-up whose address is no
longer listed; adding a way in whose verified address is not listed
(`link_error=sign_in_not_allowed`); a passkey sign-in for an account with no
way in whose address was verified and is listed; a passkey sign-up; and any
token exchange (`authorization_code` or `refresh_token`) for an account with
no such way in, answering `invalid_grant`. The sign-in request SHALL report
`passkey_sign_up` false. The refusal SHALL NOT reveal whether an account
exists.

#### Scenario: Listed domain and address
- **WHEN** the list is `example.com` or `ana@example.com` and Ana signs in with a provider-verified `ana@example.com`
- **THEN** she signs in or creates her account as usual

#### Scenario: Not listed, with an invitation
- **WHEN** an `invite_only` instance lists only `bo@example.com` and Ana has a pending invitation
- **THEN** her round trip ends at `/sign-in?error=sign_in_not_allowed` and no account, pending sign-up, token or held grant is kept

#### Scenario: Existing account off the list
- **WHEN** the list excludes the address of an existing account and its owner signs in with the provider
- **THEN** the round trip ends at `/sign-in?error=sign_in_not_allowed`, the same as for someone with no account, and no session starts

#### Scenario: Subdomain
- **WHEN** the list is `example.com` and the verified address is `a@mail.example.com`
- **THEN** it is refused

#### Scenario: Unverified address
- **WHEN** the list is set and the provider did not verify the address
- **THEN** it is refused

#### Scenario: Tightened while pending
- **WHEN** a pending sign-up was made before the list excluded its address and the person presses Create
- **THEN** the response is 403 `sign_in_not_allowed` and the pending sign-up is gone

#### Scenario: Passkey
- **WHEN** a passkey that verifies belongs to a passkey-only account, or to an account whose addresses are not listed
- **THEN** the response is 403 `sign_in_not_allowed` and no session starts; with a listed, verified address on the account it signs in

#### Scenario: Refresh after the list tightened
- **WHEN** an account's refresh token is presented after the list stopped including it
- **THEN** the token endpoint answers `invalid_grant`, and after the list includes it again the same token still refreshes

#### Scenario: Adding a way in
- **WHEN** a signed-in person links a Zoho account whose verified address is not listed
- **THEN** it is not linked and the browser returns with `link_error=sign_in_not_allowed`

#### Scenario: No list
- **WHEN** the setting is unset, empty or only commas
- **THEN** behaviour is unchanged

### Requirement: Signing up with a passkey alone
`POST /auth/passkey/sign-up/options` (CSRF) SHALL take a display name (1 to
64 printable characters, else 400 `display_name_required`) and return
creation options for a new account id, bound to the browser like a sign-in
ceremony. `POST /auth/passkey/sign-up` SHALL verify the new credential and,
in one transaction, create the account with that name and no address, its
passkey and an issuer session, publish `identity.account_created` (provider
`passkey`) and `identity.way_in_added`, and answer where to go. Both SHALL
be refused with 403 `signup_invite_only` on an invite-only instance, keeping
nothing, because a passkey proves no address an invitation could match; the
sign-in request SHALL say whether passkey sign-up is open
(`passkey_sign_up`).

#### Scenario: A new person with a passkey
- **WHEN** someone signs up with a passkey and the name "Ana Lima" on an open instance
- **THEN** an account exists with that name and no address, and the web client gets a code for it

#### Scenario: Invite only
- **WHEN** someone starts a passkey sign-up on an invite-only instance
- **THEN** the response is 403 with `code` = `signup_invite_only`, and no challenge or account is kept

### Requirement: WebAuthn verification
Registration SHALL follow WebAuthn Level 3 §7.1 and sign-in §7.2 for this
profile:
- the credential type is `public-key` and its `id` equals `rawId`;
- the client data has the ceremony's `type`, the challenge, the exact
  origin, and no `crossOrigin` true and no `topOrigin`;
- `rpIdHash` is SHA-256 of the relying-party id, user presence and user
  verification are set, and BS is set only with BE;
- a registration's attestation object is one strict CBOR data item
  (definite lengths, shortest encodings, no map key twice) with a registered
  `fmt` (`none` with an empty statement; other formats' statements are not
  trusted);
- its credential id is 16 to 1023 bytes and equals `rawId`;
- its COSE key is CTAP2-canonical CBOR, states one of the three algorithms,
  matching its key type and curve, with no private part; an Ed25519 key is a
  canonical point not of small order, a P-256 key a point on the curve, and
  RSA keys are at least 2048 bits;
- an assertion carries no attested credential data, keeps the BE it was
  registered with, and is signed over `authData || SHA-256(clientDataJSON)`
  by the stored key.

A signature counter that does not increase while either it or the stored
one is non-zero SHALL be audited (`passkey_sign_count_regressed`), not
refused, and the stored counter SHALL never decrease.

#### Scenario: A weak or malformed key
- **WHEN** a registration carries an Ed25519 identity point, a P-256 point off the curve, an attestation object with `fmt` twice, or a credential id of 15 bytes
- **THEN** it is refused with `passkey_not_accepted` and nothing is stored

#### Scenario: Every algorithm
- **WHEN** passkeys with EdDSA, ES256 and RS256 keys are registered and used
- **THEN** each registers and signs in

#### Scenario: No user verification
- **WHEN** an assertion's authenticator data has user presence but not user verification
- **THEN** it is refused

#### Scenario: A cloned authenticator's counter
- **WHEN** a passkey whose stored counter is 10 signs in reporting 10
- **THEN** the sign-in succeeds, the stored counter stays 10, and the audit trail records the regression

### Requirement: Renaming and removing a passkey
`identity.list_passkeys` (`identity:read`) SHALL list the caller's passkeys
(name, whether synced, added, last used; never keys).
`identity.rename_passkey` (`identity:write`) SHALL rename one (1 to 64
characters, else 400 `passkey_name_required`). `identity.remove_passkey`
(`identity:write`) SHALL need a recent sign-in, lock the account, refuse with
409 `last_way_in` when it is the account's only way in, end every session
signed in with it and revoke their refresh tokens, audit it and publish
`identity.way_in_removed`. Another account's passkey SHALL be 404 for all
three.

#### Scenario: Remove the only passkey of a passkey-only account
- **WHEN** a person whose only way in is a passkey removes it
- **THEN** the response is 409 with `code` = `last_way_in`

#### Scenario: Signed out with it
- **WHEN** a passkey is removed while a browser is signed in with it
- **THEN** that session's refresh token is refused, and the passkey no longer signs in

### Requirement: Passkeys section
Settings → Ways to sign in SHALL have a Passkeys card below the Accounts card:
the person's passkeys (synced or on one device, added, last used), Rename in
place, Remove behind an inline confirmation (`last_way_in` explained in
place), and "Add a passkey" (`N` then `P`), naming the new passkey after the
device. With no passkeys the card SHALL show a quiet panel, "No passkeys yet",
holding the Add button. A refusal for a stale sign-in SHALL offer "Sign in
again" through `/bff/login?max_age=600`. A browser without passkeys SHALL be
told so instead of offered Add. Every string SHALL be in en, pt-PT, es-ES
and fr, and the section usable at 375px.

#### Scenario: Add needs a fresh sign-in
- **WHEN** the person presses Add a passkey an hour after signing in
- **THEN** the section says to sign in again first and offers it, coming back to the page

### Requirement: The first Zoho mailbox comes with sign-up
A Zoho sign-in SHALL ask for `openid email profile` only. When it turns
out to be a new account and the instance has a Zoho mailbox adapter, the
browser SHALL be sent to Zoho once more, for that account, asking for the
mailbox scopes and offline access with `prompt=consent`, and the grant SHALL
be held as for Google. A connect or reconnect SHALL ask for the mailbox at
once. A refresh token Campbooks does not keep (a known account's sign-in, a
refused link or connect, a discarded held grant) SHALL be revoked at Zoho,
which caps the tokens a person holds per app. Granted scopes
SHALL be read split on spaces and commas; a Zoho token answer without
`scope` SHALL count as granting the mailbox scopes asked for.

#### Scenario: A known Zoho account signs in
- **WHEN** a person whose Zoho account is a way in signs in with Zoho
- **THEN** the authorization request asks for no mailbox and no offline access, and nothing is held

#### Scenario: A refused connect
- **WHEN** a Zoho connect is refused because that Zoho account is another person's way in
- **THEN** the refresh token Zoho issued is revoked

#### Scenario: Sign up with Zoho, then first workspace
- **WHEN** a person signs up with Zoho on an instance with a Zoho mailbox adapter, creates a workspace and adopts the held grant
- **THEN** the grant-intake port receives the grant with its refresh token and the issuer `https://accounts.zoho.eu` for a European account

### Requirement: Grants name their issuer
Every grant identity hands to the grant-intake port, held or from a connect
or reconnect, SHALL carry the issuer that vouched for the account (the ID
token's `iss`, as validated), so the receiving module can reach the
account's data centre without trusting any other input.

#### Scenario: A held grant keeps its issuer
- **WHEN** a held Zoho grant from the EU data centre is adopted a day later
- **THEN** the grant carries the issuer `https://accounts.zoho.eu`

### Requirement: Consent screen
The web client SHALL have a consent screen at `/consent?request=` in the
centred one-question layout of the sign-in screens. It SHALL name the app
and whether Campbooks or a workspace admin vouches for it ("Unverified"
otherwise), say whom the person is signed in as, list what the app will be
able to do in plain sentences, ask which workspace it may use (only its own
for a workspace's app; "all your workspaces" for Campbooks' own apps), say
where the browser goes next, warn when the app is on this device, and say
whether the answer will be remembered. A workspace's own app SHALL be named
by its workspace, and only Campbooks' own apps as Campbooks. Focus SHALL start
on Deny, no workspace SHALL be picked for the person, Allow SHALL have no
keyboard shortcut and SHALL work only once the page has been visible and
focused for a second (again after it was hidden or left), so no stray key
press or click answers it. Allow and Deny (Esc) SHALL send the answer and
follow the redirect. `step_up_required` SHALL offer "Sign in
again"; a request no longer waiting SHALL be explained. It SHALL be in en,
pt-PT, es-ES and fr, keyboard-operable with visible hints, and usable at
375px.

#### Scenario: An agent on this device
- **WHEN** the screen shows a dynamically registered agent with a loopback redirect
- **THEN** it says "Unverified", warns to allow it only if the person just started it, and Allow stays disabled until a workspace is picked

### Requirement: Apps and integrations screen
Settings SHALL have an Apps and integrations page, linked from the account
menu, listing the person's connected apps (where each works, what it can do,
when it was allowed and last used) with Revoke behind an inline
confirmation, and, for owners and admins, the open workspace's own apps with
New app (name, server-to-server or sign-in app, read or read and change, a
return address for a sign-in app), Rotate secret and Remove, each confirmed
in place. A secret SHALL be shown once with Copy and never again. Members
SHALL see that only owners and admins manage the workspace's apps. Creating
or rotating with a stale sign-in SHALL offer "Sign in again". It SHALL be in
en, pt-PT, es-ES and fr, keyboard-operable with visible hints (New app:
⇧N), and usable at 375px.

#### Scenario: Secret shown once
- **WHEN** an owner creates a server-to-server app
- **THEN** the page shows its client id and secret with Copy, and after Done the secret is gone from the page

### Requirement: Notice of a new way in
Every `identity.way_in_added` but a sign-up's first way in SHALL be told to
the account. When the instance sends mail, a notice SHALL go to the account's
primary address and to the new provider way in's address, each only when
verified and once per address, in the language the person chose (English
otherwise), naming what was added and when, with one link (the instance's
public URL) and no images or remote content. A delivery SHALL never send a
notice twice to the same address, SHALL be retried while the mail server is
unreachable, and SHALL leave an audit row with ids and counts only. Without
mail, the audit row SHALL record `no_mail_transport`, and Ways to sign in SHALL
mark the ways in added in the last seven days, never the one the account was
made with (its sign-up provider account or passkey, which `PasskeyView` says
with `signed_up_with`).

#### Scenario: Zoho added
- **WHEN** a person who signed up with Google (verified) adds a Zoho account (verified) on an instance that sends mail
- **THEN** both addresses get one notice, in the person's language, saying a Zoho account was added and when

#### Scenario: Delivered twice
- **WHEN** the same event is delivered again after the notices went
- **THEN** nothing more is sent

#### Scenario: Mail server down
- **WHEN** the mail server is unreachable at the first attempt and back later
- **THEN** the delivery is retried, the notices go, and the job's recorded error names no address

#### Scenario: Unverified address
- **WHEN** a passkey is added to an account whose primary address is not verified
- **THEN** no notice is sent

### Requirement: Signing out everywhere
`identity.sign_out_everywhere` (`identity:write`, `POST
/api/v1/identity/sign-out-everywhere`) SHALL end every live issuer session of
the caller's account and revoke every refresh token it holds, for every
client, make the API refuse every access token issued to the account so
far (see "Revoked access tokens stop at once"), record it in the audit
trail and publish `identity.signed_out_everywhere`; the web BFF SHALL end the account's web
sessions on it. It SHALL need no recent sign-in. A token bound to a workspace
SHALL get 404 and end nothing. Ways to sign in SHALL offer it behind an
inline confirmation, then sign the browser out.

#### Scenario: A stolen session
- **WHEN** a person signs out everywhere while another browser holds a session and a refresh token of their account
- **THEN** that refresh token fails with `invalid_grant`, the access tokens issued before get 401 at once, that browser's next authorization shows the sign-in screen, and its web session ends

### Requirement: A way in that came only from connecting goes with its mailbox
When a connect creates a way in (`linked_via = connect`), the identity module
SHALL record, in the transaction that creates the way in, the workspace its
mailbox is connected in, and SHALL record every later workspace that way in's
mailbox is connected in before that grant is handed over; if the record
cannot be written, the round trip SHALL fail rather than hand the grant over.
The hand-over SHALL run to its end even if the browser goes away.
When the hand-over then fails (the connectors module refuses the grant, mail
access was declined, no refresh token came back), the record that round trip
wrote SHALL be taken back and, in the same transaction under the account
lock, a way in that round trip created SHALL be removed too (as below),
unless it is the account's last way in. Once the grant is accepted, the
record SHALL name the connection it became (written if it went missing).
The connectors module SHALL publish `connectors.connection_added` for every
accepted grant, with its `provider`, `account_subject` and the grantor's
account (`grantor_subject`); `identity.name_connected_mailbox`, consuming
it, SHALL name the record of that provider account's way in in that
workspace, and only when that way in belongs to the grantor's account and
is tracked (has a record), and the connectors module, asked through the
grant-intake port, holds that very connection as the grantor's connection
of that provider account in that workspace. Replaying the event SHALL change
nothing.
A record still naming no connection a day after it was written SHALL be
checked with the connectors module through the grant-intake port: when the
connection of that provider account in that workspace, granted by that
account, exists, the prune SHALL name the record with it and leave the way
in; when it does not, the prune SHALL delete the record (if it is still
unnamed and still that old) and, under the account lock, remove a way in a
connect created that it leaves with no record (as below, unless it is the
account's last), published with reason `mailbox_never_connected`. A record
the port could not answer for SHALL be left for a later run. The prune
SHALL settle one account per transaction, in account order. A connect
retried for a record still naming no connection SHALL write that record
afresh, so it is a day old only a day after the last attempt; a named
record SHALL keep the time it was written. Should the hand-over task fail
unexpectedly, the browser SHALL still go back with a `link_error`.
`identity.forget_connected_way_in`, consuming `connectors.connection_removed`
(its workspace, connection, `provider` and `account_subject`), SHALL first
ask the connectors module, through the grant-intake port, for the record's
account's connection of that provider account in that workspace, and do
nothing while that is still the named connection (or retry while the port
cannot answer); it SHALL then lock
the account, then act only on the record naming that very connection (a
record that names none is never acted on), forget it and, when that way in
has none left, was
linked by connecting, and is not the account's last way in (provider accounts
and passkeys), SHALL remove it as
`identity.remove_way_in` does: ending every session signed in with it,
revoking their refresh tokens, auditing it and publishing
`identity.way_in_removed`. A way in with connections that were never recorded
(linked before the record existed), one linked by signing up or by Add, and
the last way in SHALL stay. Replaying the event SHALL change nothing.

#### Scenario: Its last connection goes
- **WHEN** a way in a connect created has connections in two workspaces and both are removed
- **THEN** it stays after the first removal and is gone after the second, with one `identity.way_in_removed`

#### Scenario: Another member's disconnect
- **WHEN** B connects support@ in a workspace without signing in with it, A (who made support@ a way in by connecting it in another workspace) then connects the same account in B's workspace and the grant is refused, and B later disconnects their connection
- **THEN** A's way in is untouched, and no record of A's was left in B's workspace

#### Scenario: A removal announced for a live connection
- **WHEN** `connectors.connection_removed` arrives for the only mailbox of a way in a connect created while the connectors module still holds that connection, or while it cannot answer
- **THEN** the way in, its record and its sessions stay

#### Scenario: A failed hand-over
- **WHEN** a connect adds a way in but mail access was declined at the provider, or the connectors module refuses the grant
- **THEN** the way in it added is gone again and no mailbox record is left

#### Scenario: A record the round trip never named
- **WHEN** a connect's record names no connection because the round trip ended before naming it, and `connectors.connection_added` arrives for it
- **THEN** the record names that connection, and a replay changes nothing

#### Scenario: Someone else's connection names nothing
- **WHEN** `connectors.connection_added` names the same provider account but a different grantor account
- **THEN** no record is named or written

#### Scenario: A connection that does not exist names nothing
- **WHEN** `connectors.connection_added` names the grantor's provider account but a connection the connectors module does not hold
- **THEN** no record is named, and the connection it holds still names the record

#### Scenario: A record nothing ever named
- **WHEN** a record still names no connection a day after it was written, and the connectors module holds no such connection
- **THEN** the prune deletes it and removes the way in it alone kept, with one `identity.way_in_removed`, unless that way in is the account's last

#### Scenario: A connected mailbox whose record was never named
- **WHEN** a record still names no connection a day after it was written, but the connectors module holds the grantor's connection of that account in that workspace
- **THEN** the prune names the record with it, and the way in stays

#### Scenario: A connect retried a day later
- **WHEN** a connect is retried for a record left unnamed by an attempt two days ago
- **THEN** the record is written afresh, and a prune while the retry is in flight leaves it and the way in

#### Scenario: The hand-over breaks
- **WHEN** the hand-over task panics
- **THEN** the browser goes back to where it started with `link_error=internal_error`, not a server error page

#### Scenario: Two of its mailboxes removed at once
- **WHEN** the two workspaces' mailboxes of a way in a connect created are removed concurrently
- **THEN** exactly one of the two removals removes the way in

#### Scenario: Whoever signed in with it is signed out
- **WHEN** a browser is signed in with that way in when its last connection is removed
- **THEN** that session has ended

#### Scenario: Other ways in stay
- **WHEN** the connection of a shared mailbox, of the account the person signed up with, or one never recorded is removed
- **THEN** the person's ways in are unchanged

#### Scenario: The last way in stays
- **WHEN** the only way in left is one a connect created and its connection is removed
- **THEN** it stays

### Requirement: A Microsoft mail connection goes to the mail tenant
A connect or reconnect round trip with Microsoft SHALL go to the tenant set by
`CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT` (unset: sign-in's own): its
authorization request, code exchange and ID-token validation (the issuer
computed from the token's own `tid`, as for sign-in), with the same state,
nonce, PKCE and browser binding. A sign-in and a link SHALL keep sign-in's
tenant (`CAMPBOOKS_MICROSOFT__TENANT`). An invalid mail tenant SHALL not stop
the identity module (the connectors module refuses it while Microsoft mail is
on). A connect that would add a way in SHALL add one only when sign-in's
tenant admits the account; otherwise it SHALL connect the mailbox as a shared
one is connected (no way in). Whether the round trip granted the mailbox SHALL
compare Microsoft's granted scopes without the `https://graph.microsoft.com/`
prefix and ignoring case.

#### Scenario: A personal account on an instance whose sign-in is for organizations
- **WHEN** sign-in's tenant is `organizations`, the mail tenant `common`, and a member connects an Outlook.com mailbox
- **THEN** the browser is sent to the `common` authorization endpoint, the mailbox is connected, and no way in is added

#### Scenario: The mail tenant unset
- **WHEN** sign-in's tenant is one tenant id and `CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT` is unset
- **THEN** a connect goes to that tenant's authorization endpoint

#### Scenario: Scopes answered with the Graph prefix
- **WHEN** Microsoft's token answer grants `https://graph.microsoft.com/Mail.ReadWrite https://graph.microsoft.com/Mail.Send https://graph.microsoft.com/User.Read`
- **THEN** the mailbox counts as granted

### Requirement: Signing in with Microsoft asks for mail only for a new account
A sign-in with Microsoft SHALL ask for no mailbox scope and no offline access
when the account turns out to be known, as for Zoho: Microsoft cannot revoke
one app's refresh token, and a work tenant may need an administrator's consent
for mail. Only when the account is new, and Microsoft mail is offered, SHALL a
second round trip ask for the mailbox (with consent), so the first mailbox can
come with sign-up. A connect and a reconnect SHALL always ask for it.

#### Scenario: A known account signs in
- **WHEN** a person whose Microsoft account is already a way in signs in with Microsoft on an instance that offers Microsoft mail
- **THEN** the authorization request carries no mail scope and no `offline_access`, and no grant is held

#### Scenario: A new account signs up
- **WHEN** a person signs up with a Microsoft account Campbooks doesn't know, on an instance that offers Microsoft mail
- **THEN** a second round trip asks for the mailbox with consent, and its grant is held for the new account

### Requirement: A reconnect can ask for calendar access
`identity.start_link` SHALL accept `capabilities`, of which only `calendar` is
allowed (400 `link_incomplete` for any other). The consent SHALL ask besides
the mailbox for the scopes of those capabilities and, for a `reconnect`, of
those the connection already holds. Zoho's calendar scopes SHALL still be
asked only while `CAMPBOOKS_CONNECTORS__ZOHO_CALENDAR` is on. The recent
sign-in rule of a reconnect is unchanged.

#### Scenario: Adding calendar to a Google connection
- **WHEN** a member starts a `reconnect` with `capabilities: ["calendar"]` for a Google connection that has mail only
- **THEN** the consent at Google asks for the mailbox scopes and the calendar scopes

#### Scenario: A capability that is not offered
- **WHEN** a member starts a `reconnect` with `capabilities: ["files"]`
- **THEN** the response is 400 and no ticket exists
