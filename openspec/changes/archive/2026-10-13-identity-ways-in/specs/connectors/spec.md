## REMOVED Requirements

### Requirement: Connecting an account
**Reason**: the consent now runs through the identity module's front door (ADR 0014, slice S2); there is no `complete_connection` to call.
**Migration**: clients send the browser to `start_connection`'s `authorize_url` and read the result from the redirect (`connected=` or `link_error=`); see "Connecting an account through the front door".

## ADDED Requirements

### Requirement: Connecting an account through the front door
A member SHALL start a connection by naming a provider and capabilities (and
optionally a connection of theirs to extend), and SHALL receive the URL of
the identity module's front door for that consent. The provider round trip,
its PKCE (S256), offline access and expiry are the identity module's, and
the grant SHALL arrive through the grant-intake port, which creates or
refreshes the connection through the one seam
(`create_or_refresh_connection`). Extending a connection SHALL refuse any
other account (`account_mismatch`). With no in-process identity issuer, or a
provider that is not configured, the answer SHALL be 409
`provider_not_configured`. The grant-intake port SHALL tell the identity
module, for a connection to reconnect, its provider and address, and SHALL
answer 404 unless the caller granted it.

#### Scenario: Member connects Gmail
- **WHEN** a member starts a Google connection for `messages`
- **THEN** the answer's `authorize_url` is a `/auth/start/google?link=` URL of this instance, and the grant that comes back through the intake becomes a connection with the member as grantor, health `ok` and a messages stream due now

#### Scenario: Reconnecting someone else's connection
- **WHEN** the identity module asks for the reconnect target of a connection another member granted
- **THEN** the answer is 404

#### Scenario: Same account again
- **WHEN** the member connects the same Google account again in the same workspace, asking for a capability it lacks
- **THEN** the existing connection gains the capability and its credentials are replaced; no second connection exists

#### Scenario: Reconnecting with another account
- **WHEN** a grant extending a connection comes back for a different Google account
- **THEN** it is refused with `account_mismatch` and the connection is unchanged

## MODIFIED Requirements

### Requirement: Connections operations
The module SHALL expose start, list, get, disconnect and sync-now
operations under `/api/v1/w/{workspace_id}/connections`, each with an
`operationId` prefixed `connectors.`, an `x-campbooks` block and the scope
`connectors:read` or `connectors:write`. A member SHALL see the
connections they granted; owners and admins SHALL see all of the
workspace's; any other read SHALL be 404. A client token SHALL NOT start a
connection.

#### Scenario: Member lists connections
- **WHEN** a member lists connections in a workspace with their own and a colleague's connection
- **THEN** only their own is listed, while an admin sees both

#### Scenario: Client token starts a connection
- **WHEN** a client token calls start connection
- **THEN** the response is 403 `connection_needs_user`
