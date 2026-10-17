## ADDED Requirements

### Requirement: The outbound-requests port
The module SHALL implement the kernel's outbound-requests port, read in the
asking transaction. A send SHALL stand only while its draft is `sending` as
that send, with the outgoing document recorded for it, and its author is a
member of the workspace allowed to send the draft (the check that releases
it); the draft SHALL stay locked against its outcome until the asking
transaction ends. The port SHALL return the draft's inbox and that document.
A requested change SHALL stand only when the conversation is in the named
inbox, the message is one of its messages, and its stored state shows the
change: read or unread, starred or not, in the inbox or not, the label on
the conversation or not under exactly the provider key named, the
conversation in the named stream. An item that does not parse SHALL NOT
stand.

#### Scenario: A released send stands
- **WHEN** the connector asks about a send released from Sam's draft
- **THEN** the port returns the draft's inbox and the document it was released with

#### Scenario: A send stops standing
- **WHEN** the send's author left the workspace, or its draft failed, or no document was recorded for it
- **THEN** the port says it does not stand

#### Scenario: An undone change
- **WHEN** a member marks a conversation read and then unread before the connector asks
- **THEN** the unread change stands and the read change no longer stands for that message

#### Scenario: A forged change
- **WHEN** the connector asks about an archive of a message still in the inbox, a message of another conversation, or a label under a key the label does not have
- **THEN** none of them stands

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
