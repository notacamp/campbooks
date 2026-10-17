## MODIFIED Requirements

### Requirement: A workspace's event alone is no proof
Any transaction working in a workspace can append that workspace's events,
so a consumer whose action reaches past the workspace (an account's ways in
and sessions, an app's refresh tokens, a grant at its provider) SHALL check
the state the event claims, through the owning module's port, before
acting, and SHALL do nothing while that state says otherwise. A port that
cannot answer SHALL make the delivery retry, never act on a guess.
`identity.forget_connected_way_in` (`connectors.connection_removed`) SHALL
act only when the connectors module no longer holds that connection for the
record's account; `identity.name_connected_mailbox`
(`connectors.connection_added`) SHALL name a record only with the
connection the connectors module holds for that grantor and provider
account; `identity.revoke_departed_consents` and
`connectors.revoke_departed_grants` (`workspaces.member_left`) SHALL act only
when the person is no longer a member of the workspace;
`identity.remove_workspace_clients` and `connectors.revoke_workspace_grants`
(`workspaces.workspace_deleting`) SHALL act only when the workspace is no
longer live. A consumer that acts at a provider on a member's behalf counts
as reaching past the workspace: `connectors.queue_send`,
`connectors.send_message` and `connectors.request_changes` SHALL send or
change mail only as the conversations module's outbound-requests port
confirms it. Consumers whose effects stay within the event's own workspace
(its own data, which the same transaction could change directly) SHALL NOT
be required to check.

#### Scenario: A forged removal of a live connection
- **WHEN** a transaction working in workspace A appends `connectors.connection_removed` for a connection the connectors module still holds, the only mailbox of a way in a connect created
- **THEN** the way in, its mailbox record and its sessions stay, and no `identity.way_in_removed` is published

#### Scenario: A forged connection names nothing
- **WHEN** `connectors.connection_added` names a connection the connectors module does not hold for its grantor
- **THEN** no mailbox record is named with it, and the connection it does hold still names the record

#### Scenario: A forged departure
- **WHEN** `workspaces.member_left` names someone who is still a member of that workspace
- **THEN** the apps they allowed there keep their consents and refresh tokens, and their connections there stay, unrevoked at the provider
- **AND** once they have left, the same event revokes them

#### Scenario: A forged deletion
- **WHEN** `workspaces.workspace_deleting` names a workspace that is still live
- **THEN** its clients, the consents bound to it and its connections stay, unrevoked at the provider
- **AND** once it is deleted, the same event removes and revokes them

#### Scenario: A forged send or change request
- **WHEN** a transaction working in workspace A appends `conversations.send_requested` or `conversations.changes_requested` that the conversations module's records do not bear out
- **THEN** its mailbox sends nothing and no change is made at the provider
