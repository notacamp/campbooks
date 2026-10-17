# Proposal

## Why

The round-2 security review of #207 (public #526) left four non-blocking
findings, tracked as campbooks-cloud#211:

1. **Maintenance kinds forged per workspace.** A workspace transaction may
   insert a row of its own workspace for any kind, `kernel.prune_jobs`,
   `kernel.sweep_blobs`, `bff.prune_sessions`, `identity.prune` and
   `connectors.dispatch_due` included (no schedule key, so #207's policy
   admits it). Those handlers work across workspaces on their own
   transactions or the pool, so a forged row runs an extra maintenance pass,
   attributed to no one.
2. **Handlers that trust a same-workspace event.** Any transaction working in
   workspace A can append A's events. `identity.forget_connected_way_in`
   takes `connectors.connection_removed` as proof that a connection went: a
   forged one removes a member's way in tied to A's still-live mailbox and
   ends their sessions everywhere. The same holds, in other shapes, for the
   handlers that revoke an app's refresh tokens or a provider grant on
   `workspaces.member_left` / `workspace_deleting`.
3. **Backstop edge case.** The schedule tick reads the tick from `now()`, the
   transaction's start. A tick transaction that stalls for more than a
   period finds a newer tick "far ahead" and moves it back, for one extra
   run (harmless: the unique key dedups).
4. **Instance-wide events readable from any workspace.** `instance_rows_read`
   (20261021010000) lets every workspace transaction read every instance-wide
   event and delivery, account ids included (`identity.signed_out_everywhere`,
   `identity.account_suspended`, `identity.signed_in`, ...).

Modules touched: the kernel (`jobs`, `events`, `tenancy`, one migration),
`identity`, `connectors`. No new module, dependency, endpoint, event or
scope.

## What Changes

- **Instance-only kinds.** A job kind is instance-only when it is a
  schedule's, when it delivers an instance-wide event type, or when its type
  says `INSTANCE_ONLY` (a new const on `Job` and `JobHandler`, default
  false). The worker refuses a claimed job of such a kind that carries a
  workspace: failed for good, with the reason, its handler never run.
  `Registry::instance_only_kinds` lists them.
- **Handlers re-check the state.** Six consumers whose action reaches past
  the event's workspace ask the owning module's port before acting and do
  nothing while it says otherwise: `identity.forget_connected_way_in` and
  `identity.name_connected_mailbox` ask the grant-intake port
  (`connection_of`); `identity.revoke_departed_consents` and
  `connectors.revoke_departed_grants` ask the membership port
  (`role_held`); `identity.remove_workspace_clients` and
  `connectors.revoke_workspace_grants` ask it whether the workspace is live
  (`Membership::workspace_live`, a new provided method over `resolve`).
- **The tick reads the statement's clock** (`clock_timestamp()`).
- **Workspaces read no instance-wide row.** Migration `20261021170000` drops
  `instance_rows_read` on `kernel.events` and `kernel.jobs`. The one
  workspace path that needed it, `EventBus::publish` of an event declared
  `PUBLISHED_IN_WORKSPACES` (it read the new event's id back, and Postgres
  checks a `RETURNING` against the read policies), now publishes every
  instance-wide event under an id the kernel chooses, reading nothing back.

## Non-goals

- A database check that refuses an instance-only kind's workspace row at
  insert (see `design.md`: the worker's refusal is the gate; a kind list in
  the database would have to follow every new schedule by migration).
- Re-checking consumers whose effects stay inside the event's own workspace
  (its own data, which the forging transaction could change directly);
  listed in `design.md`.
- The consumers of `conversations.*` events (sends, change requests), which
  act on the workspace's own connections: a follow-up, see `design.md`.

## Impact

- A workspace can no longer run maintenance from its own rows, remove a
  person's way in or app access by announcing a disconnect, a departure or a
  deletion that did not happen, or read anyone's account events.
- An image from before this change publishes those two account events from
  a workspace with `RETURNING` and is refused on the upgraded database: run
  the API and the worker of the same version; the rollback SQL is in
  `design.md`.
