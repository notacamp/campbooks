## ADDED Requirements

### Requirement: Microsoft mail connections
The `microsoft` provider SHALL offer the `messages` capability only, through
the front door, with one consent for `offline_access`, `Mail.ReadWrite`,
`Mail.Send` and `User.Read` (and the identity scopes every round trip asks
for), at the tenant `CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT` (`common` by
default, so personal Microsoft accounts and work and school accounts both
connect), with the OAuth client Microsoft sign-in uses
(`CAMPBOOKS_MICROSOFT__CLIENT_ID` and its secret). When the grant arrives,
connectors SHALL refuse it unless every mail scope was granted (compared
without the `https://graph.microsoft.com/` prefix and ignoring case), then
read Graph `/me` with its access token: the mailbox key SHALL be the Graph
user id (1 to 64 characters of letters, digits and `-`) and the address
`mail`, else `userPrincipalName`, trimmed and lower-cased, which SHALL parse
as an address. The connection's account SHALL stay the ID token's `sub`. The
address SHALL count as verified only when the identity module verified the
ID token's address and it is that same address. Calls SHALL go only to
`login.microsoftonline.com` and `graph.microsoft.com`, follow no redirect,
time out after 15 seconds and refuse answers over 64 KiB. Errors SHALL never
carry a token, a code or an answer's body.

#### Scenario: Member connects Outlook
- **WHEN** a member starts a Microsoft connection for `messages` and the grant comes back through the intake with every mail scope
- **THEN** a connection with provider `microsoft`, the member as grantor and health `ok` is created, its messages stream's mailbox is the Graph user id and its address the mailbox's primary address, and `connectors.connection_added` and `connectors.capability_granted` are published

#### Scenario: Same account again
- **WHEN** the member connects the same Microsoft account again in the same workspace
- **THEN** the existing connection's credentials are replaced and no second connection or mailbox exists

#### Scenario: Reconnecting with another account
- **WHEN** a grant extending a Microsoft connection comes back for another Microsoft account
- **THEN** it is refused with `account_mismatch` and the connection is unchanged

#### Scenario: Another member's account
- **WHEN** another member of the workspace connects the same Microsoft account
- **THEN** it is refused with `account_connected_by_another_member`

#### Scenario: Mail access left out
- **WHEN** the grant lacks `Mail.ReadWrite`, `Mail.Send` or `User.Read`
- **THEN** it is refused with `consent_incomplete` and nothing is stored

#### Scenario: Graph answers something it should not
- **WHEN** Graph `/me` answers without an id, with an id that is not one, with no address, or with more than 64 KiB
- **THEN** the grant is refused and nothing is stored

#### Scenario: An unverified address
- **WHEN** the ID token's address was not verified, or Graph's address is another one
- **THEN** the connection is kept with `address_verified` false

#### Scenario: A sign-in's grant
- **WHEN** a person signed up with Microsoft while Microsoft mail is offered, and adopts the held grant in their workspace
- **THEN** it becomes their Microsoft connection like a connect's grant

### Requirement: Microsoft connection health
A Microsoft connection's messages stream SHALL, until reading lands, keep its
grant alive and its health known without reading mail: each run SHALL get a
valid access token (the stored one, else a refresh at the mail tenant's token
endpoint under the per-connection refresh lock), and then wait the longest
poll interval. A refresh that Microsoft answers with `invalid_grant`,
`interaction_required`, `consent_required` or `login_required`, or with an
AADSTS error saying the grant is gone (70000, 70008, 700082, 50173, 65001,
50076, 50079, 50057, 53003), SHALL make the connection `needs_reauth`
(`grant_revoked`) and pause its streams; 429 (or a 503 with `Retry-After`)
SHALL make it `throttled` until the given time. Its outbound changes SHALL be
skipped as `unsupported`, a send through it SHALL end `not_connected`, and
disconnecting it SHALL delete its tokens without calling Microsoft.

#### Scenario: Grant revoked at Microsoft
- **WHEN** the refresh answers 400 `invalid_grant` with AADSTS70000
- **THEN** the connection's health becomes `needs_reauth` and its streams pause

#### Scenario: Throttled
- **WHEN** the refresh answers 429 with `Retry-After: 120`
- **THEN** the connection's health becomes `throttled` and the stream's next run is at least two minutes away

#### Scenario: A healthy grant
- **WHEN** the stored access token is still valid
- **THEN** the run makes no call to Microsoft and the next run is the longest poll interval away

### Requirement: Microsoft mail is off by default
`CAMPBOOKS_CONNECTORS__MICROSOFT__ENABLED` SHALL default to false. While it is
off, Microsoft SHALL not be offered as a mail connection (its mailbox scopes
are none, so the ways-in listing names no Microsoft mailbox, `start_link`
answers `mailbox_unsupported`, `start_connection` answers
`provider_not_configured`, and discovery answers `not_supported_yet`), even
where the Microsoft sign-in client is configured, and the server SHALL start
whatever the other Microsoft and connectors settings are. While it is on,
Microsoft SHALL be offered only with the Microsoft client and the instance
secret key; either missing SHALL be a warning, never a stop. Only a tenant
that is not `common`, `organizations`, `consumers` or a tenant id, with the
flag on, SHALL stop the server, naming
`CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT`.

#### Scenario: Production's settings
- **WHEN** the server starts with `CAMPBOOKS_CONNECTORS__POLL__MIN_SECONDS=60`, `CAMPBOOKS_CONNECTORS__POLL__MAX_SECONDS=120`, the Microsoft sign-in client, and no Microsoft mail variable
- **THEN** it starts and Microsoft is not offered as a mail connection

#### Scenario: Flag off with a broken tenant
- **WHEN** the flag is off and `CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT` is `not a tenant`
- **THEN** the server starts

#### Scenario: Flag on without the client
- **WHEN** the flag is on and no Microsoft client is configured
- **THEN** the server starts with a warning and Microsoft is not offered

#### Scenario: Flag on with a broken tenant
- **WHEN** the flag is on and the tenant is `not a tenant`
- **THEN** the server refuses to start, naming `CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT`

## MODIFIED Requirements

### Requirement: Connecting a mailbox by its address
The Mailboxes page SHALL offer "Add a mailbox" (⇧C), a dialog that first asks
for the mailbox's address only and then asks `connectors.discover_imap` which
way it connects: a Gmail or Google Workspace address, or a Zoho one, SHALL be
offered its provider's sign-in (the front door, as Connect is) with "Also use
this account to sign in"; an Outlook, Hotmail, Live, MSN or Microsoft 365
address SHALL be offered Microsoft's sign-in the same way ("Connect Outlook")
when discovery answers `use_provider` with `microsoft` (the instance offers
Microsoft mail), and SHALL get a note that Outlook is coming soon and nothing
to connect when it answers `not_supported_yet`; any other address SHALL get
the app-password step. When discovery answers `provider_not_configured` the
dialog SHALL offer the instance's mailbox providers instead. The app-password
step SHALL show a short how-to for the provider, chosen from discovery's
`provider_hint` or the address's domain: Yahoo, AOL, iCloud, Fastmail, GMX
and web.de (each saying to turn on IMAP in its settings first), Yandex (also
saying a new app password can take a few hours to work), and a general text
for any other domain; each named guide SHALL link to the provider's own
public help page, opened in a new tab. It SHALL have one password field and
an "Advanced" disclosure, closed unless no settings were found, holding the
username and the IMAP and SMTP host, port and security, prefilled from
discovery, where only TLS or STARTTLS can be chosen. Connecting SHALL call
`connectors.connect_imap` and SHALL explain every refusal in plain words from
its stable `code` (never the problem's `title` or `detail`), with a general
message for any other; a refusal with `step_up_required` SHALL offer "Sign in
again", as the page does. The password SHALL never be logged, SHALL use
`autocomplete="new-password"`, and SHALL be cleared from the page's state
once the request is made, on success and when the dialog closes; it SHALL
never be held in a cached mutation. The dialog SHALL trap focus, close on
Escape with nothing kept, submit each step with Enter shown on its button,
fit 375px and desktop, and be in en, pt-PT, es-ES and fr.

#### Scenario: A Gmail address
- **WHEN** a member types `ana@gmail.com` and continues
- **THEN** the dialog offers "Connect Gmail" with "Also use this account to sign in", and pressing it starts `identity.start_link` with `connect` and `google`

#### Scenario: An Outlook address on an instance that offers Microsoft mail
- **WHEN** discovery answers `use_provider` with `microsoft`
- **THEN** the dialog offers "Connect Outlook" with "Also use this account to sign in", and pressing it starts `identity.start_link` with `connect` and `microsoft`

#### Scenario: An Outlook address
- **WHEN** discovery answers `not_supported_yet` with `microsoft`
- **THEN** the dialog says Outlook is coming soon and offers nothing to connect

#### Scenario: A Yahoo address
- **WHEN** discovery answers `settings` with `provider_hint` `yahoo.com` and `password` `app_password`
- **THEN** the dialog shows Yahoo's app-password steps with a link to Yahoo's help page, one password field, and Advanced closed holding Yahoo's servers

#### Scenario: A GMX address
- **WHEN** discovery answers `settings` for `ana@gmx.net`
- **THEN** the how-to says to turn on IMAP in GMX's settings first and links to GMX's page for it

#### Scenario: Nothing found
- **WHEN** discovery answers `not_found`
- **THEN** the app-password step opens with Advanced open, its username prefilled with the address

#### Scenario: A wrong password at a provider that needs an app password
- **WHEN** `connectors.connect_imap` answers `imap_login_refused` for an address discovery said needs an app password
- **THEN** the dialog says the provider needs an app password, the password field is empty, and nothing of the server's text is shown

#### Scenario: The sign-in is too old
- **WHEN** discovery or connecting answers 403 `step_up_required`
- **THEN** the dialog says to sign in again first, and its button signs in again and comes back to the page

#### Scenario: IMAP turned off on the instance
- **WHEN** discovery answers 409 `provider_not_configured`
- **THEN** the dialog offers the instance's mailbox providers' sign-in buttons
