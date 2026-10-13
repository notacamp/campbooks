## ADDED Requirements

### Requirement: A removed connection's mail is deleted or kept read-only
On `connectors.connection_removed` with `mail=delete` (or none), the
module SHALL delete each inbox the connection fed: its messages and
attachments, conversations, labels, decisions, corrections, drafts and
memberships. It SHALL also let go of every file they held (the sweep
deletes them), remember the inbox as removed so no late batch or replayed
grant brings it back, and publish `conversations.inbox_removed`. With
`mail=keep` it SHALL mark each inbox read-only: no batch is applied to it,
nothing is written or sent from it, and its members keep reading. It SHALL
then publish `conversations.inbox_changed`, and the inbox list SHALL show
it as `read_only`.

#### Scenario: Delete
- **WHEN** a connection is removed with its mail deleted
- **THEN** no row of its inbox remains, no file reference remains, and its members get 404 for its conversations

#### Scenario: Keep
- **WHEN** a connection is removed with its mail kept
- **THEN** its members still read its conversations, archiving answers 403 `inbox_read_only`, and new batches are dropped

### Requirement: A departed member keeps nothing
On `workspaces.member_left`, the module SHALL delete the member's inbox
memberships and drafts, and stop them owning any inbox. A member who
comes back SHALL have no access to any inbox until given it again.

#### Scenario: Back in the workspace
- **WHEN** a member who left is invited back
- **THEN** they see no inbox and none of their old drafts

### Requirement: Inbox access follows the catalogue
An inbox SHALL be owned by the member whose connection feeds it. Members
see it by role: `reader` reads; `sender` also writes and sends; `manager`
also shares. Workspace owners and admins SHALL get nothing inside an inbox
they are not a member of. A draft SHALL be its author's alone: they read
and delete it while they read its inbox, and write and send it while they
may send from it.

#### Scenario: A colleague's draft
- **WHEN** the inbox's manager asks for a draft another member wrote
- **THEN** the answer is 404
