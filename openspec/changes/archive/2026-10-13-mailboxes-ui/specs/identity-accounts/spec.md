## MODIFIED Requirements

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
used), the providers configured on the instance, and, as
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
