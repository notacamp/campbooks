## Context
`identity.mailbox_links` records which workspaces a connect-made way in's
mailbox is connected in (campbooks-cloud#117, #68). A record is named with
its connection by the round trip and by `identity.name_connected_mailbox`.
The prune sweep treats a record unnamed for a day as a connect that died
before its grant was handed over.

## Decisions

### Ask the connectors module before removing (chosen)
The sweep asks, through the grant-intake port the two modules already
share, whether the connection exists for that workspace, provider account
and grantor (the account, by its token subject, resolved to a member by the
connectors module, as `adopt` and `reconnect_target` do). Found: the record
is named with it; that is what both namings would have done. Not found: the
record is deleted and the way in may go. An error leaves that record alone
until the next run, and a way in with any record left is never removed.
The question is per workspace (the connectors module reads its own rows
under that workspace's row-level security), so no cross-workspace read is
added.

*Considered:*
1. **Skip ways in with a pending or dead `name_connected_mailbox` job.** It
   reads the kernel's job table by consumer and payload, which no module
   does today, and says nothing once a job is gone: a dead job may have
   been for a connection that exists or one since removed, and no job at
   all may mean a connect that died or an event still being published.
   Rejected: it guesses where the port knows.
2. **Never remove a way in from the sweep, only the record.** Leaves a way
   in nothing can ever remove (the #117 rule's hole). Rejected.

### A retried connect writes its record afresh
`record` upserts with `ON CONFLICT … DO UPDATE SET created_at = now WHERE
connection_id IS NULL`, so a round trip in flight always has a record
younger than the grace period. The deletion re-checks `created_at <
cutoff` and `connection_id IS NULL` under the account lock, so a record
written afresh between the port's answer and the deletion is not deleted.
`RETURNING xmax = 0` still tells an insert (this round trip's to take back)
from an existing row (another round trip's).

### One account per transaction
The sweep used to lock every stale account in one transaction, in whatever
order the query returned them. It now settles each account in its own
transaction, accounts in `account_id` order, so it holds one account lock
at a time and never waits on a lock while holding another.

### A panic in the hand-over
The hand-over runs in a spawned task so a dropped request cannot cancel it
halfway; a panic there surfaced as a bare 500 on the provider callback. It
now redirects back with the generic `link_error=internal_error`, which the
clients show as their generic "wasn't connected" message. A record it left
unnamed is settled by the sweep.

## Risks
- The port adds one in-process query per stale record per sweep. Stale
  records are rare (both namings must fail), so this is negligible.
