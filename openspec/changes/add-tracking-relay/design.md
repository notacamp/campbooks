# Design

## The relay loop

One task per worker process. Several processes may run it at once.

1. Open a transaction in the kernel scope and claim up to `BATCH_SIZE`
   unpublished rows, oldest id first, with `FOR UPDATE SKIP LOCKED`. The
   claim goes through a definer function (`kernel.claim_tracked_events`),
   because the runtime role can only append to the outbox. Another worker
   skips locked rows, so each row goes to one publisher at a time.
2. Publish each row as one message, then wait for every confirm.
3. Mark the confirmed rows published (`kernel.mark_tracked_events_published`,
   a definer function), then commit. A nack or a timeout rolls the whole
   batch back, so it is published again later. A crash between confirm and
   commit publishes a row twice, and subscribers deduplicate on the event id.
4. Loop at once while batches come back full. Otherwise wait for
   `NOTIFY kernel_tracked_events` (sent by `track`'s insert, through a
   trigger) or `POLL_INTERVAL_MS`, whichever comes first.
5. On a broker error, reconnect with exponential backoff (1 s up to 60 s,
   with jitter), and log a WARN line carrying the error kind only (never the
   URL or its credentials).

## The message

- Exchange `EXCHANGE` (topic, durable); routing key `<name>.v<version>`.
- Body: JSON `{ envelope..., "body": { properties and records } }`, i.e. the
  envelope's fields at the top level and the row's body under `body`.
- Properties: `message_id` = the event id, `type` = the name,
  `content_type` = `application/json`, `timestamp` = `occurred_at`,
  `delivery_mode` = persistent, header `x-event-version`.

## Topology

When `DECLARE_TOPOLOGY` is on: exchange `EXCHANGE` (topic, durable), queue
`STREAM` (`x-queue-type: stream`, `x-max-age: STREAM_MAX_AGE`, durable),
bound with `#`. Declarations are idempotent, so a restart changes nothing.
With it off, the relay only publishes, which fits a least-privilege user with
write but no configure permission.

## Data ownership

Migration `<ts>_kernel__tracked_events_relay.sql`: the two definer functions
(owned by the definer role, as the kernel's other definer functions are,
with `search_path` pinned), the `NOTIFY` trigger, and the outbox's purge
grant if the purge needs one. `kernel.prune_tracked_events` is a kernel
schedule, like `kernel.prune_events`.

## Metrics (no ids, per the telemetry rules)

`campbooks.tracking.published{outcome=confirmed|nacked|error}` (counter),
`campbooks.tracking.backlog` (gauge, unpublished rows),
`campbooks.tracking.oldest_unpublished_seconds` (gauge). The last two are
sampled the way queue depth is.

## Events published or consumed

None (domain events). This change publishes tracked events to the broker.
