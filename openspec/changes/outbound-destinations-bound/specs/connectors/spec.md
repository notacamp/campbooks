## ADDED Requirements

### Requirement: Sends and changes go only where they were bound to go
The connector SHALL answer the kernel's outbound-destinations port: an
inbox's destination is its `messages` stream, that stream's connection and
its mailbox at the provider, and no destination when no stream, or more than
one, claims the inbox. Before it records a send, and again before every call
to the provider for it, the connector SHALL pass the outbound-requests port
the destination it would send through (the recorded send's stream, the
stream's connection and its mailbox), and SHALL send only while the port says
the send stands for it. When it records an outbound change it SHALL bind
each row, with a MAC labelled `campbooks/connectors.outbound-change/v2`, to
the stream, the stream's connection and its mailbox as resolved then (the
inbox's one stream; nothing is recorded when two claim it); a run SHALL make
a change only when its MAC verifies for the connection and mailbox the run
loaded. A send refused this way SHALL fail `unavailable` without a provider
call, and a change SHALL end `skipped` (`unverified`) with its message
re-read; each with a WARN line carrying ids and counts only, and neither
retried.

#### Scenario: A stream repointed after a send was released
- **WHEN** SQL points a released send's stream at another mailbox before the connector sends it
- **THEN** the provider receives no call and the send fails `unavailable`, with no job left retrying it

#### Scenario: A stream repointed before the send is recorded
- **WHEN** SQL points the inbox's stream at another mailbox between the release and the connector recording the send
- **THEN** no send is recorded and nothing is sent

#### Scenario: A send released where it goes
- **WHEN** a send is released for the inbox's stream as it is
- **THEN** it is sent once

#### Scenario: A stream repointed after a change was recorded
- **WHEN** a member archives a message and SQL then points the stream at another mailbox before the run
- **THEN** the run writes nothing to the provider and the change ends `skipped` (`unverified`)

### Requirement: Where a trashed message goes back to is bound
When a run puts a message in the trash at Zoho and records where it was
(`restore_to`), it SHALL bind that place into the change's MAC, only while
the change still verifies as recorded. Before an untrash or a not-spam (out
of the trash or spam and into neither) uses a recorded place, the run SHALL
verify it; when it does not verify, the change SHALL NOT be made: it SHALL
end `skipped` (`unverified`), the message SHALL be re-read, and a WARN line
carrying ids and counts only SHALL be logged.

#### Scenario: A place edited with SQL
- **WHEN** a member trashes a Zoho message filed in one of their folders, SQL edits where it goes back to, and the member untrashes it
- **THEN** Zoho receives no move, the message stays in the trash and the untrash ends `skipped` (`unverified`)

#### Scenario: Trash and back
- **WHEN** nothing edits the place and the member untrashes the message
- **THEN** it goes back to the member's folder

### Requirement: What was bound before destinations were
The migration that started binding destinations SHALL record each `messages`
stream's inbox, connection and mailbox, and each outbound change's
`restore_to`, in tables the runtime role may read and never write; a stream
or a change removed takes its record with it. A send or change MAC of the
earlier encoding (`…/v1`) SHALL verify only while the destination resolved
now is the one recorded for its stream, and a place of the earlier encoding
only while it is also the one recorded for its change. A MAC of the new
encoding SHALL never verify as one of the earlier.

#### Scenario: A change in flight at the deploy
- **WHEN** a change bound before the deploy is pending and its stream still goes where it went then
- **THEN** the run makes it

#### Scenario: A change in flight, repointed
- **WHEN** such a change's stream was pointed at another mailbox since the deploy, or the stream has no record
- **THEN** the run makes nothing and the change ends `skipped` (`unverified`)

#### Scenario: Writing the record
- **WHEN** a transaction of the runtime role inserts or updates a row of the record
- **THEN** it is refused
