## ADDED Requirements

### Requirement: The outbound-requests port
The module SHALL implement the kernel's outbound-requests port, read in the
asking transaction. A send SHALL stand only while its draft is `sending` as
that send, with the outgoing document recorded for it, and its author is a
member of the workspace allowed to send the draft (the check that releases
it); the draft SHALL stay locked against its outcome until the asking
transaction ends. The port SHALL return the draft's inbox and that document.
Every `conversations.changes_requested` the module publishes SHALL be
recorded, in the same transaction, under the event's id with its inbox,
conversation and items (ids and provider message ids only); a record SHALL
be deleted with its conversation, and a record older than seven days when
the next request on that conversation is recorded. A requested change SHALL
stand only when the request of that id, conversation and inbox holds an
equal item. An item that does not parse SHALL NOT stand.

#### Scenario: A released send stands
- **WHEN** the connector asks about a send released from Sam's draft
- **THEN** the port returns the draft's inbox and the document it was released with

#### Scenario: A send stops standing
- **WHEN** the send's author left the workspace, or its draft failed, or no document was recorded for it
- **THEN** the port says it does not stand

#### Scenario: A change and its undo
- **WHEN** a member marks a conversation read and then unread before the connector asks
- **THEN** both requests stand, each under its own id, and the connector merges them

#### Scenario: A forged change
- **WHEN** the connector asks about a request id the module never recorded, an item the request did not hold, or a label key other than the one requested
- **THEN** none of them stands, even a change the conversation already shows

## MODIFIED Requirements

### Requirement: The outbound intent
When the window ends, the job `conversations.release_send` SHALL write an
outgoing message document (`contracts/outgoing-message.v1.schema.json`, a
blob holding the addresses, subject, plain text, the HTML rendered from the
text's basic formatting with everything else escaped, and the forwarded
attachments' blob references), mint a send id and a `Message-ID`, and
publish `conversations.send_requested` v1
(`contracts/conversations.send_requested.v1.schema.json`, ids and the blob
only) in the transaction that marks the draft as sending, which SHALL also
record the document's blob reference with the send, for the
outbound-requests port to return.

#### Scenario: Formatting cannot inject HTML
- **WHEN** the body contains `<script>` and `**done**`
- **THEN** the HTML part shows `&lt;script&gt;` as text and `done` in bold
