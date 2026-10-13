## MODIFIED Requirements

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
