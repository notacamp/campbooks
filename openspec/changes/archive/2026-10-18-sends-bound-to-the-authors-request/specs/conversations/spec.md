## ADDED Requirements

### Requirement: Sends are bound to the author's request
A send SHALL be bound to the authenticated request that asked for it with a
MAC under a key derived from the instance secret (HKDF-SHA256, label
`campbooks/send-binding/v1`), which SHALL never be stored in the database,
logged or sent. `conversations.send_draft` SHALL store a MAC over the
workspace, the draft, the send generation it schedules, the author, the
inbox and a hash of everything that goes out (the sender, To, Cc and Bcc,
subject, body and quoted history, reply headers and thread, what it
answers, and each attachment with its blob hash, size and type), encoded
canonically (a versioned label, each field length-prefixed). Before it
writes anything, `conversations.release_send` SHALL verify that MAC against
the draft as stored, in constant time; a draft whose MAC does not verify,
or that has none, SHALL NOT be released: no document written, no change to
the draft, a WARN line carrying ids only, and no retry. The release SHALL
store a second MAC over the workspace, the send id, the inbox, the author
and the outgoing document's blob, and clear the first; cancelling a send
SHALL clear it too. Every change request the module records SHALL carry a
MAC over the workspace, the request, the inbox, the conversation and its
items. Without an instance secret nothing SHALL be signed or verify:
`send_draft` SHALL refuse (500) and nothing SHALL be sent.

#### Scenario: A draft planted with SQL
- **WHEN** a transaction of the runtime role in a workspace inserts a scheduled draft of its own, with a member who may send from the inbox as author and a matching generation, and plants a `conversations.release_send` job for it, with no MAC, a made-up one or another draft's
- **THEN** nothing is released, no document is written, the draft is left as it was and no `conversations.send_requested` is published

#### Scenario: A scheduled draft edited with SQL
- **WHEN** a member's scheduled draft has its recipients, body, subject, reply headers or attachments changed, or is moved to another generation, inbox or author, and a release runs for it
- **THEN** it is not released; put back as the member scheduled it, it is released when its window ends

#### Scenario: Undo and send again
- **WHEN** Sam cancels a send and sends the draft again
- **THEN** the first binding is cleared, the second send carries a new one and is released once

#### Scenario: No instance secret
- **WHEN** the server has no `CAMPBOOKS_SECRET_KEY`
- **THEN** it logs a WARN at startup, `send_draft` answers 500 and no scheduled draft is released

#### Scenario: Drafts scheduled before the binding existed
- **WHEN** the migration that adds the binding runs while drafts are scheduled
- **THEN** those drafts go back to drafts, and their authors send them again

## MODIFIED Requirements

### Requirement: The outbound-requests port
The module SHALL implement the kernel's outbound-requests port, read in the
asking transaction. A send SHALL stand only while its draft is `sending` as
that send, with the outgoing document recorded for it, its stored-send MAC
verifies (over the workspace, send id, inbox, author and that document),
and its author is a member of the workspace allowed to send the draft (the
check that releases it); the draft SHALL stay locked against its outcome
until the asking transaction ends. The port SHALL return the draft's inbox
and that document. Every `conversations.changes_requested` the module
publishes SHALL be recorded, in the same transaction, under the event's id
with its inbox, conversation, items (ids and provider message ids only) and
its MAC; a record SHALL be deleted with its conversation, and a record older
than seven days when the next request on that conversation is recorded. A
requested change SHALL stand only when the request of that id,
conversation and inbox holds an equal item and the request's MAC verifies.
An item that does not parse SHALL NOT stand. A send or a request whose MAC
does not verify SHALL NOT stand, with a WARN line carrying ids only.

#### Scenario: A released send stands
- **WHEN** the connector asks about a send released from Sam's draft
- **THEN** the port returns the draft's inbox and the document it was released with

#### Scenario: A send stops standing
- **WHEN** the send's author left the workspace, or its draft failed, or no document was recorded for it
- **THEN** the port says it does not stand

#### Scenario: A stored send edited or planted with SQL
- **WHEN** a sending draft's MAC is removed or made up, its document is replaced with another blob, its author is changed, or a sending draft is planted with a copy of a real one's document and MAC under another send id
- **THEN** the port says it does not stand, and the connector sends nothing

#### Scenario: A change and its undo
- **WHEN** a member marks a conversation read and then unread before the connector asks
- **THEN** both requests stand, each under its own id, and the connector merges them

#### Scenario: A forged change
- **WHEN** the connector asks about a request id the module never recorded, an item the request did not hold, or a label key other than the one requested
- **THEN** none of them stands, even a change the conversation already shows

#### Scenario: A change request edited or planted with SQL
- **WHEN** a recorded request's items are changed, its MAC removed, or a request is planted under a new id with a real one's MAC
- **THEN** none of its items stands
