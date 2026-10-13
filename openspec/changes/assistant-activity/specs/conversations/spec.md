## ADDED Requirements

### Requirement: Triage is recorded as activity
When `conversations.apply_changes` stores new inbound messages, it SHALL
record, in its transaction, one `conversations.triage` session for the batch
within the inbox, triggered by `sync`, with each inbound message as a
subject, a done rules step per message (its answers as non-personal facts and
its decisions as references), and the model stage pending for each message
the model pass will read (the `decide` role configured and the message
inside the backfill window). `conversations.decide_message` SHALL record its
model step for the message: working before it calls the model, then done
with the answers in the transaction that stores them, skipped when a pending
message is already answered or gone, and failed with a stable error code on
its last attempt. A replayed delivery that finds nothing pending SHALL record
nothing. Batches with no inbound message SHALL record no session.

#### Scenario: Without the decide role
- **WHEN** a batch is applied and the `decide` role is not configured
- **THEN** its session has only rules steps, no pending stage, and is `done`

#### Scenario: Outbound mail only
- **WHEN** a batch brings only messages the member sent
- **THEN** no session is recorded

### Requirement: Answers about a set of messages
`GET /api/v1/w/{workspace_id}/decisions` (`conversations.list_decisions`,
`conversations:read`) SHALL take 1 to 100 message ids (else 400) and return,
for each message in an inbox the caller is a member of, its subject, sender,
conversation, stream and stream source, every answer of every strategy about
it with its source, value, confidence, reasons with their evidence and
whether it acted, and the corrections on record for its conversation.
Messages the caller may not see, or that no longer exist, SHALL be omitted.

#### Scenario: Two inboxes
- **WHEN** a member of the Work inbox only asks about one Work message and one Home message
- **THEN** the response holds the Work message only

#### Scenario: Too many
- **WHEN** a client asks about 101 messages
- **THEN** the response is 400
