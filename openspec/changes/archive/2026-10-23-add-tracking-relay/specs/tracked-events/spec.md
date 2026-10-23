## ADDED Requirements

### Requirement: The outbox is published to RabbitMQ
With `CAMPBOOKS_TRACKING__SINK=amqp`, the worker role SHALL publish every
outbox row to the configured topic exchange with routing key
`<name>.v<version>`, the event id as `message_id`, and the envelope and body
as JSON. It SHALL mark a row published only after the broker confirms it.

#### Scenario: An archived conversation reaches the stream
- **WHEN** Sam archives a conversation with the sink `amqp`
- **THEN** one message with routing key `conversations.conversation_archived.v1` and Sam's event id as `message_id` can be read from the stream, and the outbox row is marked published

#### Scenario: The broker is down
- **WHEN** the broker is unreachable while events are tracked
- **THEN** the rows stay unpublished, the relay retries with backoff, and once the broker is back every row is published

#### Scenario: Two workers relay at once
- **WHEN** two worker processes run the relay against the same outbox
- **THEN** each row is published, and no row is held by both at once

### Requirement: The relay declares its topology when asked
With `CAMPBOOKS_TRACKING__DECLARE_TOPOLOGY` on, the relay SHALL declare the
exchange, a stream queue kept for `STREAM_MAX_AGE`, and their binding,
idempotently. With it off, it SHALL declare nothing.

#### Scenario: A restart
- **WHEN** the relay starts against a broker where the topology exists
- **THEN** it starts publishing and changes nothing

### Requirement: Published rows are pruned
`kernel.prune_tracked_events` SHALL delete published rows older than
`CAMPBOOKS_TRACKING__PUBLISHED_RETENTION_HOURS`, and SHALL never delete an
unpublished row. Purging a workspace SHALL delete its outbox rows.

#### Scenario: The broker has been down for days
- **WHEN** rows have been unpublished for longer than the retention
- **THEN** the prune keeps them
