# Tasks

## 1. The kernel
- [x] 1.1 Migration: `kernel.claim_tracked_events`, `kernel.mark_tracked_events_published` (definer, pinned search_path), the `NOTIFY` trigger, purge/prune access
- [x] 1.2 Config `amqp` and its settings; the AMQP URL never logged
- [x] 1.3 `tracking::relay`: claim → publish → confirms → mark, in one transaction; LISTEN + poll; reconnect backoff; topology declaration
- [x] 1.4 `kernel.prune_tracked_events` schedule; the workspace purge deletes outbox rows
- [x] 1.5 Metrics: published, backlog, oldest unpublished

## 2. The server
- [x] 2.1 Start the relay in `worker` and `all` when the sink is `amqp`; stop it on shutdown with the worker's grace period

## 3. CI and docs
- [x] 3.1 RabbitMQ service container in the Rust test job; tests skip with a clear message only when `CAMPBOOKS_TEST_AMQP_URL` is unset locally, and CI sets it
- [x] 3.2 `docs/tracking.md` (relay, message format, how to subscribe), `deploy/.env.example`, self-hosting docs

## 4. Tests
- [x] 4.1 A tracked event reaches the stream with its routing key, `message_id` and body; its row is marked published
- [x] 4.2 A nacked or failed publish leaves the batch unpublished; after reconnecting, it is published
- [x] 4.3 Two relays: every row published, none held by both at once
- [x] 4.4 Declaring twice is a no-op; with declaring off, nothing is declared
- [x] 4.5 The prune keeps unpublished rows and deletes published ones past retention; a purged workspace loses its rows

## 5. Last
- [x] 5.1 Changelog entry file (`changelog/unreleased/added/321-tracking-relay.md`)
- [x] 5.2 Gates green in CI; recording
- [x] 5.3 Archive the change
