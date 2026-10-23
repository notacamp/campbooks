## MODIFIED Requirements

### Requirement: Credentials stay secret
Refresh tokens, access tokens and mailbox passwords SHALL be stored only
encrypted (AES-256-GCM, key derived from the instance secret with a purpose
label, bound to table, column and row) and SHALL never be returned by any
operation, written to logs, or carried in events or job payloads. Configuring
a provider without the instance secret SHALL stop startup with an error
naming the variable.

#### Scenario: Reading a connection
- **WHEN** a member reads a connection
- **THEN** the response holds its provider, address, capabilities, health and streams, and no token or password

#### Scenario: Ciphertext moved to another row
- **WHEN** a sealed refresh token is copied into another connection's row
- **THEN** it does not decrypt, and that connection's health becomes `needs_reauth`

#### Scenario: Password moved to another row
- **WHEN** a sealed IMAP password is copied into another connection's row
- **THEN** it does not decrypt, and that connection's health becomes `needs_reauth`

## ADDED Requirements

### Requirement: Finding a mailbox's servers
`connectors.discover_imap` SHALL find IMAP and SMTP settings for an email
address from, in order, the bundled provider database, the domain's own
autoconfig document (HTTPS only, through the host guard, no redirects), DNS
SRV records and the domain's MX host, without sending the address or its
domain to any third party. An address whose mail is hosted by a provider
Campbooks connects natively SHALL be answered with that provider instead of
IMAP settings. It SHALL be limited to 30 calls per member per hour.

#### Scenario: A big provider
- **WHEN** a member asks for `ana@yahoo.com`
- **THEN** the answer holds Yahoo's IMAP and SMTP servers from the bundled database and says an app password is needed

#### Scenario: A custom domain at a hosting company
- **WHEN** a member asks for `ana@bakery.example` and `autoconfig.bakery.example` publishes a configuration
- **THEN** the answer holds those servers with source `autoconfig`

#### Scenario: A custom domain on Google
- **WHEN** a member asks for an address whose domain's MX is at Google
- **THEN** the answer is `use_provider: google` and no IMAP settings

#### Scenario: Nothing found
- **WHEN** no source yields settings
- **THEN** the answer is `not_found` and the member can enter the servers by hand

### Requirement: Connecting an IMAP mailbox
`connectors.connect_imap` SHALL connect an IMAP mailbox only after signing
in to its IMAP server and authenticating to its SMTP server with the given
password, over TLS or STARTTLS with verified certificates, and SHALL keep
nothing when either fails. It SHALL require a member token, `connectors:write`,
permission to create a connection, and a sign-in within the last ten
minutes. Refusals SHALL name what went wrong with a stable code and SHALL
never repeat the server's own text. The connection SHALL carry the
`messages` capability and SHALL never be a way to sign in.

#### Scenario: App password accepted
- **WHEN** a member who signed in five minutes ago connects `ana@yahoo.com` with a valid app password
- **THEN** the connection is created with provider `imap`, its `messages` stream starts, and `ConnectionAdded` is published

#### Scenario: Wrong password
- **WHEN** the IMAP server refuses the login
- **THEN** the answer is `imap_login_refused`, nothing is stored, and for Yahoo it adds that an app password is needed

#### Scenario: Server without TLS
- **WHEN** the IMAP server on port 143 does not offer STARTTLS
- **THEN** the answer is `starttls_missing` and no password was sent

#### Scenario: Old sign-in
- **WHEN** the member signed in more than ten minutes ago
- **THEN** the answer is 403 `step_up_required`

#### Scenario: Client token
- **WHEN** an API client token calls the operation
- **THEN** the answer is `connection_needs_user`

#### Scenario: Reconnecting with a new password
- **WHEN** the member who connected a mailbox reconnects it with a new valid password
- **THEN** the same connection keeps its streams, the new password replaces the old one, and its health is `ok`

#### Scenario: Turned off
- **WHEN** `CAMPBOOKS_CONNECTORS__IMAP__ENABLED` is false
- **THEN** both IMAP operations answer `provider_not_configured` and `imap` is not offered

### Requirement: Mail servers are reached safely
Every connection to a member-supplied host (IMAP, SMTP, autoconfig) SHALL
resolve the host, refuse it unless every address is public (not loopback,
private, link-local, CGNAT, metadata, documentation, multicast or their IPv6
forms), and connect to the address it checked. Only ports 993, 143, 465 and
587 SHALL be used. A self-hosted instance MAY allow private hosts and other
ports by setting.

#### Scenario: Metadata address
- **WHEN** a member enters an IMAP host that resolves to `169.254.169.254`
- **THEN** the answer is `host_refused` and no connection is opened

#### Scenario: Rebinding
- **WHEN** a host resolves to a public address at the check and a private one a moment later
- **THEN** Campbooks connects to the public address it checked

#### Scenario: Self-hosted mail server
- **WHEN** `CAMPBOOKS_CONNECTORS__IMAP__ALLOW_PRIVATE_HOSTS` is true and the host is `10.0.0.5`
- **THEN** the connection is attempted

### Requirement: Connecting cannot be used to guess passwords
Connect attempts SHALL be limited to 10 per member and 30 per workspace per
hour, and failed logins to 5 per target address per hour across all
workspaces, after which attempts for that address answer
`too_many_attempts` for an hour.

#### Scenario: Guessing one mailbox
- **WHEN** five connect attempts for `bob@example.net` fail within an hour, from any workspaces
- **THEN** the sixth answers `too_many_attempts` without contacting the server
