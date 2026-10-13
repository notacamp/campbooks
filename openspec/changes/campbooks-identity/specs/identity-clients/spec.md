## Purpose

Which clients may obtain tokens: first-party apps, clients a workspace
registers, and clients that register themselves (MCP agents), each with the
consent and workspace binding it needs.

## ADDED Requirements

### Requirement: First-party clients
The web client `campbooks-web` (confidential) and the native client
`campbooks-native` (public) SHALL exist on every install without
provisioning. The native client SHALL accept loopback redirects on any port
and the private-use scheme `com.notacamp.campbooks:/oauth/callback`, and
SHALL use PKCE S256 with no client secret. First-party clients SHALL NOT
show a consent screen.

#### Scenario: Native loopback on any port
- **WHEN** the native client sends `redirect_uri=http://127.0.0.1:53124/callback`
- **THEN** the authorization proceeds and the code is delivered to that port

### Requirement: Workspace clients
Owners and admins of a workspace SHALL be able to register, list, rotate the
secret of and remove clients for that workspace, with scope
`identity:clients`. A confidential client's secret SHALL be shown once and
stored only as a hash. A `client_credentials` token SHALL carry the
client's `workspace_id`, and SHALL be refused by any workspace route naming
another workspace (404). Members SHALL get 404 on these operations.

#### Scenario: Machine client bound to its workspace
- **WHEN** a client registered in workspace W calls a route of workspace V with a client_credentials token
- **THEN** the response is 404

#### Scenario: Member cannot register clients
- **WHEN** a member (not owner or admin) calls `identity.register_client`
- **THEN** the response is 404

### Requirement: Consent for third-party clients
Before issuing a code to any client that is not first-party, the issuer
SHALL show a consent screen naming the client, whether it is verified, the
full redirect host, each requested scope in words, and a workspace picker
listing the person's workspaces. The resulting tokens SHALL carry the chosen
`workspace_id`, and every workspace route SHALL require a token carrying
`workspace_id` to name the path's workspace. Consents SHALL be remembered per
account, client, workspace and scopes, and revocable; revoking one SHALL
revoke the refresh tokens it produced.

#### Scenario: Agent limited to one workspace
- **WHEN** a person consents to an agent for workspace W and the agent calls a route of workspace V where the person is also a member
- **THEN** the response is 404

#### Scenario: Revoked consent
- **WHEN** the person revokes the agent's consent
- **THEN** the agent's refresh token fails with `invalid_grant`

### Requirement: Dynamic and metadata-document clients
The issuer SHALL accept RFC 7591 registrations of public clients only
(`token_endpoint_auth_method` = `none`, authorization code with PKCE,
`https` or loopback redirect URIs), rate-limited per address, and SHALL
delete such a client after 30 days without use. It SHALL accept an `https`
`client_id` with a path as a Client ID Metadata Document: it SHALL fetch it
only from a public address, without following redirects, within 5 seconds
and 64 KiB. It SHALL require the document's `client_id` to equal the URL, and
SHALL take redirect URIs only from the document. Such clients SHALL be shown
as unverified on the consent screen, with a warning when their only
redirects are loopback.

#### Scenario: Metadata document on a private address
- **WHEN** a `client_id` URL resolves to a private or link-local address
- **THEN** the authorization fails with `invalid_client` and no request is sent

#### Scenario: Confidential dynamic registration refused
- **WHEN** a registration asks for `token_endpoint_auth_method=client_secret_basic`
- **THEN** it fails with `invalid_client_metadata`
