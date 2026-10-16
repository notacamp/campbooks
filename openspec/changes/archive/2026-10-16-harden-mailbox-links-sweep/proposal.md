## Why
The prune sweep deletes a mailbox record still naming no connection a day
after it was written, and removes the connect-made way in it leaves behind.
It assumes "unnamed for a day" means "never connected". That is not so when
both namings failed (the round trip's fast path, and
`identity.name_connected_mailbox` with its retries spent or still pending):
the mailbox is connected, and the sweep would take a way in the person still
uses. A connect retried a day after an earlier one died also reuses the old
row with its old `created_at`, so the sweep could delete it while the retry
is in flight.

## What Changes
- The grant-intake port gains `connection_of(subject, workspace, provider,
  account_subject)`: the connection of that provider account in that
  workspace when that person granted it (none for anyone else, or once they
  left the workspace). The connectors module answers from its own rows.
- Before deleting, the sweep asks the port for each stale record: one whose
  connection exists is named, not deleted, and its way in stays. A record
  the port could not answer for is left for the next run.
- `record` writes a still-unnamed row afresh (`created_at = now`) when a
  connect is retried, so the sweep waits for that round trip too; a named
  row keeps its time.
- The sweep settles one account per transaction, in `account_id` order.
- A panic in the hand-over task sends the browser back with
  `link_error=internal_error` instead of a bare 500.

## Capabilities
- Modified: `identity-accounts`, `connectors`.

## Impact
Modules: `identity`, `connectors`, and the kernel's grant-intake port (one
new method). No migration, no new event or operation, no client change.
Tracking: notacamp/campbooks-cloud#148.
