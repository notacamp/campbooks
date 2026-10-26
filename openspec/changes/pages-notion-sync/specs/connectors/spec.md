## ADDED Requirements

### Requirement: Connecting Notion
With `CAMPBOOKS_CONNECTORS__NOTION__ENABLED` on and a Notion client
configured, a member SHALL be able to connect their own Notion account with
the `pages` capability through an authorization-code flow owned by
connectors: a `state` bound to the browser, single use, valid for 10
minutes. The connection SHALL belong to that member, SHALL be identified by
Notion's `bot_id`, and SHALL keep its tokens sealed by the instance key and
bound to the connection row. Every refresh SHALL store the new access and
refresh tokens together, and an `invalid_grant` SHALL mark the connection as
needing a reconnect. A client token SHALL NOT start the flow. While the flag
is off, no Notion route or hook SHALL exist and no Notion setting SHALL stop
the server.

#### Scenario: Connecting
- **WHEN** Marta connects Notion and shares "Product roadmap" on Notion's consent screen
- **THEN** a Notion connection with the `pages` capability is stored as Marta's, and its tokens are readable only as that connection's

#### Scenario: A replayed callback
- **WHEN** a Notion callback arrives with a `state` that was already used
- **THEN** it is refused and no connection is stored

### Requirement: Notion changes reach Campbooks
Each linked Notion page SHALL have a change stream that reads the page's
blocks when the page is due, and emits a normalized snapshot: block ids,
kinds, attributes, rich-text runs, edit times and content hashes, with kinds
Campbooks lacks as `external`. Notion files SHALL be stored as blobs within
20 MiB and an allowed type. `POST /hooks/notion` SHALL exist whenever Notion
is on, before any webhook secret is configured. With a webhook secret set,
a post SHALL be accepted only with a valid `X-Notion-Signature` over its raw
body, SHALL only mark the matching streams due, and SHALL never call Notion
itself; a verification-shaped body SHALL then be answered 404. With no
webhook secret set, the only body taken SHALL be exactly
`{"verification_token": <text of at most 200 characters>}`: it SHALL be kept
sealed with the instance key as the instance's one token (a newer one
replaces it), answered 200 and logged without its value; every other post
SHALL be answered 404. The token SHALL be readable only through the
instance-admin CLI (`campbooks-server admin notion-webhook-token`, with
`--clear` to forget it), never through an HTTP endpoint, an event or a log.
Without a webhook secret, connections SHALL poll. Calls SHALL go only to `api.notion.com`, follow no
redirect, and stay within the connection's request budget.

#### Scenario: A forged webhook post
- **WHEN** a post to `/hooks/notion` carries a wrong signature
- **THEN** it is refused, and no stream is marked due

#### Scenario: Notion verifies the subscription
- **WHEN** no webhook secret is set and Notion posts `{"verification_token": "..."}` to `/hooks/notion`
- **THEN** the token is kept sealed, the post is answered 200, and the operator reads the token with the admin CLI

#### Scenario: A post that is not a verification token, before the secret is set
- **WHEN** no webhook secret is set and a post carries anything but a bare verification token
- **THEN** it is answered 404 and nothing is kept

#### Scenario: A verification token after the secret is set
- **WHEN** a webhook secret is set and a verification-shaped body is posted
- **THEN** it is answered 404 and no token is kept

### Requirement: Campbooks changes reach Notion
Notion block updates, appends and deletes SHALL be made only from outbound
changes bound by a MAC to their workspace, stream, Notion page, block and
kind, and verified before the call. A block whose Notion edit time moved
since the recorded base SHALL NOT be written. A Notion page SHALL never be
archived or deleted by Campbooks.

#### Scenario: Notion changed it first
- **WHEN** Campbooks is about to update a block that Rui edited in Notion after the last sync
- **THEN** the update is not sent, and the next inbound run raises a conflict for that block
