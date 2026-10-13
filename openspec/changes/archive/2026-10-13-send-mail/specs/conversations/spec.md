## ADDED Requirements

### Requirement: Sending scope and roles
The `conversations` module SHALL declare the scope `conversations:send`, and
every draft operation SHALL require it. A user SHALL also need the `sender`
or `manager` role on the draft's inbox: a `reader` SHALL get 403
`inbox_read_only`, and a user who is not a member of the inbox SHALL get 404.

#### Scenario: A token without the scope
- **WHEN** a client with only `conversations:read conversations:write` creates a draft
- **THEN** the response is 403 as `application/problem+json`

#### Scenario: The composer
- **WHEN** a member presses Reply, Reply all or Forward in the reader, or `c` for a new message
- **THEN** a composer opens From the inbox, with To, Cc and Bcc as chips, the quoted history folded under the text, forwarded attachments listed, ⌘↵ to send, ⌘B and ⌘I to format and Esc to close, and Undo in a toast after sending

#### Scenario: A reader replies
- **WHEN** a member with the `reader` role on an inbox creates a reply draft to one of its messages
- **THEN** the response is 403 with code `inbox_read_only` and no draft exists

### Requirement: Drafts
A draft SHALL belong to one inbox and to its author, and SHALL be visible
only to its author (a client token: only to client tokens); anyone else
SHALL get 404. Creating a reply, reply-all or forward draft of a message
SHALL fill on the server: the recipients (reply: the message's `Reply-To`,
else its sender, or for a message the inbox sent its `To`; reply all: also
its `To` and `Cc`, never the inbox's own address, no duplicates; forward:
none), the subject with `Re: ` or `Fwd: ` unless already there, the
`In-Reply-To`, the `References` (the message's plus its `Message-ID`, at most
20), the provider thread key, and for a forward every non-inline attachment
of the message, whose blobs the draft holds. A draft SHALL be saved by
partial updates that name the `version` they change; a stale version SHALL
get 409 `draft_changed`.

#### Scenario: Reply all leaves the inbox out
- **WHEN** Sam replies all to a message from Maya to Sam and Leo, copying Ana
- **THEN** the draft is to Maya and copies Leo and Ana, and Sam's own address is in neither list

#### Scenario: Forward keeps the attachments
- **WHEN** Sam forwards a message with a PDF attachment
- **THEN** the draft has no recipients, its subject starts with `Fwd: `, and it lists the PDF, which stays stored even if the original message is removed

#### Scenario: Two saves of the same version
- **WHEN** two updates name the same version
- **THEN** the first is saved and the second answers 409 `draft_changed`

#### Scenario: Another member's draft
- **WHEN** a second sender of the inbox reads Sam's draft
- **THEN** the response is 404

### Requirement: Sending with undo
`conversations.send_draft` SHALL refuse a draft with no recipient (400
`no_recipients`), a malformed address (400 `invalid_address`) or more than
25 MiB of attachments (400 `attachments_too_large`); otherwise it SHALL schedule the
draft and answer 202 with the time it will be released, after
`CAMPBOOKS_CONVERSATIONS__UNDO_SEND_SECONDS` (default 10). Until then
`conversations.cancel_send` SHALL put it back to an editable draft; after
release it SHALL answer 409 `too_late`. A scheduled or sending draft SHALL
not be edited, deleted or sent again (409 `draft_not_editable`).

#### Scenario: Undo within the window
- **WHEN** Sam sends a reply and cancels it before the window ends
- **THEN** the draft is editable again and nothing is ever published for that send

#### Scenario: Sending again after an undo
- **WHEN** Sam cancels a send, edits the draft and sends it again
- **THEN** only the second send is released, once

### Requirement: The outbound intent
When the window ends, the job `conversations.release_send` SHALL write an
outgoing message document (`contracts/outgoing-message.v1.schema.json`, a
blob holding the addresses, subject, plain text, the HTML rendered from the
text's basic formatting with everything else escaped, and the forwarded
attachments' blob references), mint a send id and a `Message-ID`, and
publish `conversations.send_requested` v1
(`contracts/conversations.send_requested.v1.schema.json`, ids and the blob
only) in the transaction that marks the draft as sending.

#### Scenario: Formatting cannot inject HTML
- **WHEN** the body contains `<script>` and `**done**`
- **THEN** the HTML part shows `&lt;script&gt;` as text and `done` in bold

### Requirement: The send outcome
On `connectors.send_completed` v1
(`contracts/connectors.send_completed.v1.schema.json`), the consumer
`conversations.record_send` SHALL delete a sent draft, releasing its blobs,
and publish `conversations.draft_sent`; or mark a failed draft `failed` with
the reason and publish `conversations.send_failed`. A failed draft SHALL be
editable and may be sent again. The sent message SHALL reach its
conversation through the connector's change batch, like any message.

#### Scenario: The reply appears in its conversation
- **WHEN** a reply is sent
- **THEN** the conversation gains one outbound message and the draft is gone

#### Scenario: Gmail refuses the message
- **WHEN** the connector reports `rejected`
- **THEN** the draft is `failed` with reason `rejected` and can be edited and sent again
