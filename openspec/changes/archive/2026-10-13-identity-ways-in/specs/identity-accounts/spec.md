## MODIFIED Requirements

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

## ADDED Requirements

### Requirement: Adding a way in while signed in
`identity.start_link` (`identity:write`) with purpose `link` SHALL need a
configured provider and a sign-in within the last ten minutes, and SHALL
return a URL of `/auth/start/{provider}?link=<ticket>`. The ticket SHALL be
single use, expire after two minutes, and start a round trip only in a
browser whose issuer session belongs to the same account; otherwise nothing
SHALL start and the browser SHALL return with `link_error=sign_in_again` or
`link_error=link_expired`. The round trip SHALL ask for consent again
(`prompt=consent`). Completed, it SHALL link the provider identity to the
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
(404 otherwise) and no recent sign-in. Both SHALL ask for identity, mailbox
and offline access with `prompt=consent`, link the identity as for `link`,
and then hand the grant to the grant-intake port for that workspace (naming
the connection for `reconnect`). A refusal of the port SHALL return the
browser with `link_error=<its code>` and keep the way in; success SHALL
return it with `connected=<connection id>`.

#### Scenario: Connect a second Gmail
- **WHEN** a signed-in member connects a second Google account in their workspace
- **THEN** that Google account is a way in, and the grant-intake port receives its grant with a refresh token for that workspace

#### Scenario: Reconnect after a revoked grant
- **WHEN** a member's connection needs re-authorisation and they reconnect it
- **THEN** the provider is asked for that account (`login_hint`), the port receives the grant naming that connection, and no recent sign-in was needed

#### Scenario: Someone else's connection
- **WHEN** a member starts a `reconnect` for a connection a colleague granted
- **THEN** the response is 404

### Requirement: Listing and removing ways in
`identity.list_ways_in` (`identity:read`) SHALL list the caller's provider
ways in (provider, address as given at link time, how and when linked, last
used) and the providers configured on the instance.
`identity.remove_way_in` (`identity:write`) SHALL need a recent sign-in,
lock the account, and refuse with 409 `last_way_in` when the way in is the
account's only one, counting provider identities and passkeys. A way in of
another account SHALL be 404. Removing SHALL NOT disconnect any mailbox,
SHALL be audited, and SHALL publish `identity.way_in_removed`.

#### Scenario: Remove the last way in
- **WHEN** a person with only a Google way in removes it
- **THEN** the response is 409 with `code` = `last_way_in` and the way in remains

#### Scenario: Remove one of two
- **WHEN** a person with a Google and a Zoho way in removes the Zoho one
- **THEN** it is gone, signing in with Zoho now leads to the new-account screen, and every mailbox connection is untouched

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
Settings SHALL have a Ways to sign in page listing the person's provider ways
in, with Remove behind an inline confirmation (and `last_way_in` explained
in place), Add for each configured provider, Reconnect for this workspace's
connections of theirs that need re-authorisation, a "Sign in again" offer
when an action needs a recent sign-in, and further sections other slices
contribute (passkeys). The sign-in screen SHALL offer every provider the
server lists. Both SHALL be in en, pt-PT, es-ES and fr, keyboard-operable
with visible hints, and usable at 375px.

#### Scenario: Remove with confirmation
- **WHEN** the person presses Remove on a way in
- **THEN** an inline confirmation names the provider and address and says the mailbox stays connected, and nothing is removed until they confirm

#### Scenario: Microsoft and Zoho offered
- **WHEN** the server lists `google`, `microsoft` and `zoho` for a sign-in request
- **THEN** the sign-in screen shows the three, in that order, with `1`, `2` and `3`
