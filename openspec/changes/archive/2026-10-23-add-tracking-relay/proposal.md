# Proposal

## Why

campbooks-cloud#321, step 2 of epic #319. #320 writes tracked events to the
`kernel.tracked_events` outbox, in the transaction of each change. This change
publishes them to RabbitMQ (ADR 0020), so the subscribers that come next (the
warehouse, #323, and the admin event view, #324) can read them.

Modules touched: the kernel (the relay in `tracking/`, its config, a definer
function, the prune schedule, the workspace purge) and the server (starting
the relay in the worker role). New dependency: `lapin` (AMQP 0-9-1). One
kernel migration. No new endpoint, scope or event.

## What Changes

- `CAMPBOOKS_TRACKING__SINK=amqp`, with `__AMQP_URL` (a secret),
  `__EXCHANGE` (default `campbooks.events`), `__STREAM` (default
  `campbooks.events`), `__STREAM_MAX_AGE` (default `30D`),
  `__DECLARE_TOPOLOGY` (default `true`), `__BATCH_SIZE` (default 200),
  `__POLL_INTERVAL_MS` (default 1000) and `__PUBLISHED_RETENTION_HOURS`
  (default 24).
- A relay in the `worker` role (and `all`) that publishes the outbox to a topic
  exchange with publisher confirms. A row is marked published only after the
  broker confirms it. While the broker is down, rows wait and the relay backs
  off.
- On start, it declares the exchange, the stream and the binding when
  `DECLARE_TOPOLOGY` is on, and does nothing when it is off (the hosted cloud
  declares them with infrastructure, #322).
- `kernel.prune_tracked_events` (hourly) deletes published rows after their
  retention. Unpublished rows are never pruned. A purged workspace's rows are
  deleted with it.
- Metrics: published count by outcome, the backlog, and the age of the oldest
  unpublished row.
- CI's test job gets a RabbitMQ service container.

## Non-goals

- The broker's infrastructure on staging and prod (#322), and subscribers
  (#323, #324).
- Ordering guarantees beyond "in id order per batch": events from concurrent
  transactions can commit out of order. Consumers order by `occurred_at`
  and deduplicate on the event id.
- The RabbitMQ stream protocol. The relay publishes over AMQP 0-9-1 to an
  exchange bound to a stream queue, so no plugin is needed.
