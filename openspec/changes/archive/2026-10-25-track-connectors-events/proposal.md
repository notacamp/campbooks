# Track the connectors module's changes

Module: `connectors` (`crates/modules/connectors/`: its operations, the
sync and send jobs, its schedule and event handlers, and the IMAP IDLE
pool), with the tracking plan (`tracking/`). Tracking:
https://github.com/notacamp/campbooks-cloud/issues/329, part of epic
https://github.com/notacamp/campbooks-cloud/issues/319.

## Why

ADR 0020 asks for a record of every action: who did it, in what context,
and what the records it touched looked like then. The connectors module is
where Campbooks acts at a provider on someone's behalf: it connects a
mailbox, reads it, makes the changes members ask for, sends their mail,
writes their calendar changes, and notices when a grant stops working.
None of it is tracked yet, so the record cannot answer "when did this
mailbox stop syncing", "did that send fail, and why" or "who disconnected
it". Full coverage becomes a CI failure with campbooks-cloud#332, so every
operation, job, schedule and handler of the module needs a Coverage row
first.

## What Changes

- Tracking plan version 6 (`tracking/patches/0006-connectors.yaml`): 10
  new live events of `connectors` (`connection_added`,
  `connection_reconnected`, `connection_removed`,
  `connection_health_changed`, `connection_sync_requested`,
  `stream_synced`, `outbound_changes_made`, `calendar_change_completed`,
  `send_succeeded`, `send_failed`), the records `connectors.connection`,
  `connectors.stream` and `connectors.send`, their enums, and a Coverage
  row for each of the module's 4 mutating operations, 3 jobs, 1 schedule
  and 6 event handlers. The identity operations that hand a grant to the
  module (`identity.finish_sign_in`, `identity.adopt_held_grant`) list the
  connection events too.
- Each change is tracked with `Tracker::track` (a job's `ctx.track`) in the
  transaction that makes it, with a snapshot read in that transaction. A
  change that changes nothing tracks nothing.
- A sync run is tracked once, as a summary with counts (`stream_synced`),
  and only when it stored a batch or moved the stream's phase; the
  outbound changes a run made at the provider are tracked once per run,
  with counts by how they ended (`outbound_changes_made`). Never one event
  per message.
- Tests read every event back from `kernel.tracked_events`. The module's
  test kit writes the outbox, so every existing test runs the tracking
  code too.

No new endpoint, scope, migration, dependency or domain event. No change to
what any endpoint returns.

## Non-goals

- The other modules (#325, #326, #328, #330, #331) and the hard coverage
  gate (#332).
- One event per message read, per batch or per label written: unbounded;
  the run's summary carries the counts.
- Transient trouble: a retried send, a run that waits for its budget, a
  token refreshed, a run backing off after an error that does not change
  the connection's health. Only the connection's health changing is
  tracked.
- A calendar stream's own trouble reason (`calendar_api_not_enabled`): it
  marks the stream, not the connection, and a member cannot act on it.
- Any personal data or content: no address, display name, host, username,
  folder or label name, subject, provider id or provider error text.
