## ADDED Requirements

### Requirement: Streams mirrored to the provider
Each received message SHALL store the stream its upsert says it is in at
the provider (`provider_stream`). The module SHALL request `move_to_stream`
to the conversation's stream for every received message of a conversation
whose stored provider stream differs from it (no value counting as
different) when the model pass places the conversation, and for a received
message that joins a conversation already placed (stream source not
`default`). Messages received before their inbox's first sync completed
SHALL never be mirrored. Requests SHALL go through the recorded, bound
request path of every other change.

#### Scenario: Scout's sort reaches Gmail
- **WHEN** a message arrives in Gmail's Promotions tab and the model pass places its conversation in `primary`
- **THEN** `changes_requested` carries `move_to_stream` `primary` for it

#### Scenario: Already where Scout put it
- **WHEN** a message arrives in Gmail's Updates tab and the model pass places its conversation in `updates`
- **THEN** no `changes_requested` is published

#### Scenario: A reply joins a sorted conversation
- **WHEN** a new message in Gmail's Updates tab joins a conversation in `newsletters`
- **THEN** `changes_requested` carries `move_to_stream` `newsletters` for it

#### Scenario: The backfill is left alone
- **WHEN** the model pass places conversations from a new inbox's 90-day backfill
- **THEN** no `changes_requested` is published for them

## MODIFIED Requirements

### Requirement: Applying change batches
The job `conversations.apply_changes` SHALL apply a `MessageChangeBatch` v1
(`contracts/message-change-batch.v1.schema.json`, stored as a blob, the job
holding only the inbox and the blob reference) in order, idempotently per
`(inbox, provider_message_id)`: an unknown message is inserted and threaded,
a known one has its read, starred, mailbox state, labels and provider
stream updated, and a removed one is deleted with its blobs detached.
When a known received message's provider stream changes to a stream (a move
in the member's mail client), its conversation SHALL move to that stream
with stream source `member` unless it is already there, and the module
SHALL request `move_to_stream` to it for the conversation's other received
messages whose stored provider stream differs, and for that message when
its upsert says its stream mark is stale. An upsert without a provider
stream SHALL leave the stored one unchanged and move nothing. Bodies,
attachments and the
message's participants document SHALL be attached to the message in the
transaction that writes it, and the events SHALL be published in that same
transaction. A batch for an inbox that does not exist yet SHALL be retried; a
batch of another version or naming another inbox SHALL fail for good.

#### Scenario: Applying the same batch twice
- **WHEN** the same batch is applied again
- **THEN** no message, conversation, decision or event is added

#### Scenario: A later batch changes flags and removes a message
- **WHEN** a batch marks a message read and starred, takes another out of the inbox and removes a third
- **THEN** each affected conversation is recounted, one with no messages left is deleted, and `conversations.conversation_changed` is published with origin `provider`

#### Scenario: Moved in the mail client
- **WHEN** a batch says a known message of a `newsletters` conversation is now in the provider's `primary`
- **THEN** the conversation is in `primary` with stream source `member`, `conversation_changed` is published with origin `provider` and change `stream`, and `changes_requested` carries `move_to_stream` `primary` for its other received messages still elsewhere at the provider

#### Scenario: Our own write comes back
- **WHEN** a batch says a known message is now in the provider's `primary`, its conversation being in `primary`
- **THEN** only the stored provider stream changes, and no event is published

#### Scenario: The provider's sort on arrival
- **WHEN** a batch inserts a new message the provider placed in `updates`
- **THEN** its provider stream is stored and its conversation is placed by the decide pass, not by the provider

#### Scenario: The inbox is not there yet
- **WHEN** a batch arrives for an inbox this module has not created
- **THEN** the job is retried rather than failed, and the batch blob stays held

#### Scenario: A malformed batch
- **WHEN** a batch does not parse
- **THEN** the job fails for good and its stored error names only the kind and position of the error, never content from the batch

### Requirement: Streams from accepted answers
Every conversation SHALL be in exactly one stream: `primary`, `updates`,
`receipts` or `newsletters`. A new conversation's stream SHALL be set from
its first inbound message, only from an answer to the `stream` question that
the policy accepts; otherwise it SHALL be `primary`. The facts never answer
`stream`, so a conversation is stored in `primary` and the model pass places
it: when the policy accepts the model's `stream` answer for the
conversation's first inbound message and the conversation's stream source is
still `default`, or `rules` (a placement by the content rules retired in
ADR 0018), the conversation SHALL move to that stream with stream source
`model`. Later messages SHALL never re-sort a conversation, and a member's
move, made in Campbooks or in their mail client, SHALL stick. The
provider's own sort of a new message SHALL never place a conversation.

#### Scenario: No rules strategy
- **WHEN** the module runs with no `decide` role
- **THEN** every conversation is in `primary` with stream source `default`

#### Scenario: The shipped policy
- **WHEN** the facts strategy runs on a message with mailing-list headers
- **THEN** it gives no `stream` answer and the conversation is stored in `primary`

#### Scenario: A policy that lets rules place the stream
- **WHEN** a test policy accepts rules stream answers from 0.8 and a test rules strategy answers `newsletters` at 0.95
- **THEN** the conversation is in `newsletters` with stream source `rules`, and the answer is stored as acted on with its reasons

#### Scenario: The model sorts a newsletter
- **WHEN** the policy accepts model stream answers from 0.9 and the model answers `newsletters` at 0.98 for a conversation's first inbound message
- **THEN** the conversation is in `newsletters` with stream source `model`, and the answer is stored as acted

#### Scenario: A placement by the retired rules gives way
- **WHEN** the content rules placed a conversation in `receipts` before the upgrade, and the model pass answers `newsletters` for its first inbound message with an accepted confidence
- **THEN** the conversation is in `newsletters` with stream source `model`

#### Scenario: A member's move sticks
- **WHEN** a member moved a conversation to `primary` before the model pass answers `newsletters` with an accepted confidence
- **THEN** the conversation stays in `primary` and the answer is stored with `acted = false`

#### Scenario: A move in the mail client sticks
- **WHEN** a member moved a message to Gmail's Primary tab before the model pass answers `newsletters` with an accepted confidence
- **THEN** the conversation stays in `primary` and the answer is stored with `acted = false`

### Requirement: Tidying conversations
`conversations.mark_read` / `mark_unread`, `star` / `unstar`,
`archive` / `unarchive` and `add_label` / `remove_label` SHALL be undo pairs,
and `move_to_stream` SHALL be its own undo. Each SHALL change only what is not
already so, and when it changes something SHALL publish
`conversations.conversation_changed` with origin `member` and
`conversations.changes_requested` (`contracts/conversations.changes_requested.v1.schema.json`)
listing each affected message's provider id and the change kind. Changes that
came from the provider SHALL never produce `changes_requested`, except the
`move_to_stream` requests that bring a conversation's other messages, and a
stale stream mark, to the stream the provider reported.

#### Scenario: Marking read twice
- **WHEN** a member marks a conversation read, then read again
- **THEN** one `changes_requested` with a `mark_read` item per previously unread message is published, and none the second time

#### Scenario: Removing a provider label
- **WHEN** a member takes off a label the provider knows
- **THEN** the `remove_label` item carries the label's provider key

#### Scenario: A provider change is not sent back
- **WHEN** a batch says a known message was archived and starred at the provider
- **THEN** no `changes_requested` is published
