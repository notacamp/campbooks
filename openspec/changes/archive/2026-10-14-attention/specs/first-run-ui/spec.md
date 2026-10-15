## MODIFIED Requirements

### Requirement: Empty Home
While the user can see no inbox in the workspace, its Home SHALL say that
nothing needs the user yet and show the keyboard shortcuts available on the
page; once they can see one, Home SHALL show what needs them instead
(home-ui). The empty Home's mail section SHALL offer the person's held
mailbox grant when there is one, else name the mailboxes connected to the
workspace that the person can see, else offer "Connect your mail" as
unavailable with copy saying that connecting more mailboxes is coming.

#### Scenario: New workspace
- **WHEN** a user with no held mailbox and no connection opens a workspace's Home
- **THEN** "Nothing needs you yet" is shown and the "Connect your mail" button is disabled with a note that it is coming

#### Scenario: Mailbox connected
- **WHEN** `connectors.list_connections` returns a connection for `ana@example.com` and its inbox is not listed yet
- **THEN** Home says that `ana@example.com` is connected

#### Scenario: The inbox is there
- **WHEN** `conversations.list_inboxes` lists an inbox for the user
- **THEN** Home shows what needs them instead of "Nothing needs you yet"
