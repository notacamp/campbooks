## ADDED Requirements

### Requirement: Only the member who connected a mailbox disconnects it
Disconnecting SHALL be allowed only to the member who connected the
account. Owners and admins see it (and may sync it now), but SHALL get 403
`disconnect_owner_only`. A client token gets 403 `connection_needs_user`.
Who may see it stays the grantor, owners and admins, and client tokens.

#### Scenario: An admin disconnects someone's mailbox
- **WHEN** a workspace admin asks to disconnect a member's connection
- **THEN** the answer is 403 `disconnect_owner_only` and nothing is removed

### Requirement: The mail of a removed connection is deleted unless kept
`DELETE …/connections/{id}` SHALL take `mail=delete` (the default) or
`mail=keep`. `connectors.connection_removed` SHALL carry the choice and the
member who had connected it
(`contracts/connectors.connection_removed.v1.schema.json`). When a member
leaves or is removed, their connections SHALL be removed with the choice
made by the owner or admin who removed them (`delete` when they left on
their own).

#### Scenario: Keep on disconnect
- **WHEN** the grantor disconnects with `mail=keep`
- **THEN** `connectors.connection_removed` says `"mail": "keep"`

### Requirement: The dispatcher only reads and leases the schedule
A transaction that sets no workspace SHALL be able to read the scheduling
index and lease due streams, and nothing else. It SHALL NOT be able to
insert or delete schedule rows.

#### Scenario: The policies
- **WHEN** the database's policies are listed
- **THEN** the only module policies admitting a transaction without a workspace are the schedule's `FOR SELECT` and `FOR UPDATE` ones
