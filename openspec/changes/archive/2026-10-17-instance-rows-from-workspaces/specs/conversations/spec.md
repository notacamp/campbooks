## MODIFIED Requirements

### Requirement: The needs backfill's dispatcher reaches only its own index
The needs backfill's dispatcher (`conversations.dispatch_needs_backfills`)
SHALL work in the maintenance scope (a job's `maintenance_tx`, allowed for
it under `[[access.maintenance_scope]]` in `architecture.toml`), in which it
SHALL be able to read the pending and running backfills (ids and states)
and start pending ones, and nothing else: no kernel table admits that
scope, so it SHALL enqueue each inbox's page job in the inbox's workspace,
set for that enqueue only (`jobs::enqueue_to_in`) in the same transaction,
so that nothing after it runs in the last inbox's workspace. A transaction
that set neither a workspace nor the maintenance scope, or that is only in
the kernel scope, SHALL see no backfill row.

#### Scenario: Backfills of two workspaces
- **WHEN** inboxes of two workspaces have pending backfills and the dispatcher ticks, as the runtime role
- **THEN** each backfill is started and its first page job is enqueued in its own workspace

#### Scenario: No workspace left set
- **WHEN** the dispatcher has enqueued the page jobs of inboxes of two workspaces in its transaction
- **THEN** the transaction works in no workspace

#### Scenario: The kernel scope
- **WHEN** a transaction of the runtime role in the kernel scope reads `conversations.needs_backfills`
- **THEN** it sees no row
