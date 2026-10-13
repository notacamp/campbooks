## ADDED Requirements

### Requirement: Sending through Gmail
On `conversations.send_requested` v1 the consumer `connectors.queue_send`
SHALL record the send, keyed by its send id (a repeated delivery adds
nothing), hold the outgoing message document, and enqueue
`connectors.send_message`. That job SHALL build an RFC 5322 message from
the document (From the connection's account, To, Cc, Bcc, Subject, Date,
`Message-ID`, `In-Reply-To`, `References`, a plain-text part, an HTML
alternative when given, and the attachments) and send it with
`users.messages.send` in the document's thread. When the inbox has no
connected stream, or its grant needs the member again, the send SHALL fail
with `not_connected` or `needs_reauth` without calling Gmail.

#### Scenario: A reply goes into its Gmail thread
- **WHEN** a reply's document names a thread key
- **THEN** Gmail receives one `messages.send` with that `threadId`, and the raw message carries the `In-Reply-To`, `References` and `Message-ID` of the document

### Requirement: Sends are idempotent
Every call to Gmail SHALL be preceded by an attempt committed on its own.
When an attempt ends unclearly (a timeout, a 5xx, the worker died), the next
attempt SHALL wait at least 30 seconds and then look the message up by its
`Message-ID`, recording it as sent if Gmail has it, and sending only if it
does not. A definite refusal SHALL fail the send with `rejected` at once. A
send whose last attempt ends unclearly SHALL fail with `unknown_outcome`.

#### Scenario: A timeout after Gmail accepted the message
- **WHEN** Gmail accepts a message but the answer never arrives, and the job runs again
- **THEN** the job finds the message by its `Message-ID` and Gmail receives no second `messages.send`

#### Scenario: The same intent delivered twice
- **WHEN** `conversations.send_requested` is delivered twice for one send id
- **THEN** one send is recorded and Gmail receives one `messages.send`

#### Scenario: A refused message
- **WHEN** Gmail answers 400 to `messages.send`
- **THEN** the send fails with `rejected` and is never retried

### Requirement: The sent message comes back once
After Gmail accepts a message, the job SHALL, in one transaction, index it
like a synced message, store and announce a `MessageChangeBatch` holding it
(`connectors.messages_changed`), mark the send sent, release its document,
publish `connectors.send_completed` v1 with the provider's id, and make the
stream due at once. The next sync SHALL recognise the message and not emit
it as new.

#### Scenario: Sync after a send
- **WHEN** the stream syncs after a send and Gmail's history lists the sent message as added
- **THEN** no second upsert of that message is emitted as new and the conversation still has one copy
