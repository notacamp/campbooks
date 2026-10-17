# Design

## 1. Instance-only kinds

### The kinds, and the scope each runs in

`Registry::instance_only_kinds()` derives the list; on today's server:

| Kind | Why instance-only | Runs in |
|---|---|---|
| `kernel.prune_jobs`, `kernel.prune_events`, `kernel.prune_idempotency` | schedule | the kernel scope, one `begin_kernel` per batch |
| `kernel.prune_changes` | schedule | the kernel scope (`ctx.kernel_tx()`, which already refuses a workspace's job) |
| `kernel.prune_rate_limits` | schedule | the pool, through the kernel's definer function |
| `kernel.sweep_blobs` | schedule | the kernel scope, inside the blob store |
| `bff.prune_sessions` | schedule | the pool, through the BFF's definer functions |
| `identity.prune` | schedule | the pool (identity's tables carry no workspace) and the grant-intake port |
| `connectors.dispatch_due` | schedule | the maintenance scope, enqueueing each run in its workspace |
| `conversations.dispatch_needs_backfills` | schedule | the maintenance scope (`ctx.maintenance_tx()`, which already refuses a workspace's job) |
| `bff.end_signed_out_sessions`, `bff.end_suspended_sessions` | deliver an instance-wide event | the pool, through the BFF's definer function |
| `identity.notify_way_in_added` | delivers an instance-wide event | the pool, and the mailer |
| `workspaces.announce_member_suspended`, `workspaces.announce_member_unsuspended` | deliver an instance-wide event | fan out per workspace |

No code enqueues any of them with a workspace: schedules enqueue from the
tick (kernel scope, no workspace), and a delivery carries its event's
workspace, none for an instance-wide type. `kernel.sequence_changes` is a
workspace's job and is not on the list.

### Where the refusal sits

`KindSpec`/`KindInfo` carry `instance_only`. It is set by:

- `Background::schedule::<J>` (always, whatever `J` says);
- `on_event`/`consumer` when `!E::WORKSPACE_SCOPED` (matching what
  `load_event` already refuses for such a delivery, now before decoding);
- `job::<J>` / `handler::<H>` from the new `INSTANCE_ONLY` const (default
  false; nothing sets it yet, it is there for an instance-only job that is
  not scheduled).

The worker checks it in `execute`, after looking the kind up and before
building the `JobContext`: a job of an instance-only kind with a workspace
is marked failed for good (`JobError::Fatal`, "`<kind>` runs instance-wide
only; a job of it in a workspace is refused, not run"), logged at WARN, and
its handler never runs. It stays in the queue as a failed job for the
admin, pruned with the other failed jobs. The heal of #207 (a schedule row
of a workspace holding the schedule's key) is unchanged.

### No insert-time database check (judgement call)

Considered: a kernel table of instance-only kinds (like
`kernel.instance_rows_from_workspaces`) read by `workspace_isolation`'s
`WITH CHECK`, so the row is refused at insert. Not done:

- the worker's refusal is the security gate: a forged row never runs, and
  writes nothing but its own failed state;
- a workspace may already insert any number of rows of its own of any
  workspace kind (that is how its own work is enqueued), so refusing these
  few at insert removes no capability, only some failed rows in Admin → Jobs;
- the table would have to follow every new schedule and every consumer of an
  account event by migration, kept equal by a server test, a second list to
  forget, and every workspace job insert would pay one more lookup.

## 2. Handlers that act on an event from a workspace

Any transaction working in workspace A can append A's events (row-level
security admits A's own rows, and `load_event` only checks the event was
logged in its own workspace). A consumer whose action stays within A
(A's own data, which that transaction could change directly) gains an
attacker nothing; one whose action reaches past A must check the state the
event claims. The audit, of every consumer of `connectors.*`,
`workspaces.*` and `identity.*` events:

| Consumer ← event | What it does | Reaches past the workspace? | Now |
|---|---|---|---|
| `identity.forget_connected_way_in` ← `connectors.connection_removed` | removes the account's way in, ends its sessions everywhere | yes | re-checks: `GrantIntake::connection_of` for the record's account; nothing while it still returns the named connection; a port error retries |
| `identity.name_connected_mailbox` ← `connectors.connection_added` | names the record with a connection id | yes: a record naming a connection that never was lets a forged removal of it take the way in | re-checks: names only when `connection_of` for the grantor returns that very connection |
| `identity.revoke_departed_consents` ← `workspaces.member_left` | revokes their consents bound to A and every refresh token those clients hold for the account; removes A's clients they registered | yes (the refresh tokens) | re-checks: `Membership::role_held` in the delivery's transaction; nothing while they are a member |
| `identity.remove_workspace_clients` ← `workspaces.workspace_deleting` | removes A's clients; revokes consents bound to A and those clients' refresh tokens | yes (the refresh tokens) | re-checks: `Membership::workspace_live`; nothing while A is live |
| `connectors.revoke_departed_grants` ← `workspaces.member_left` | revokes their grants at the provider, deletes their connections | yes (a provider grant is the account's, not the workspace's) | re-checks: `role_held` in a short transaction of A |
| `connectors.revoke_workspace_grants` ← `workspaces.workspace_deleting` | revokes every grant at the provider, deletes A's connections | yes | re-checks: `workspace_live` |
| `conversations.forget_connection` ← `connectors.connection_removed` | deletes A's inbox data | no | unchanged |
| `conversations.create_inbox` ← `connectors.capability_granted`, `conversations.ingest_messages` ← `connectors.messages_changed` | A's own inboxes and mail | no | unchanged |
| `conversations.forget_member`, `assistant` / `attention` / `search` / `conversations` purges ← `workspaces.member_left` / `workspace_deleting` | delete or schedule deleting A's own data | no | unchanged |
| `conversations.pause_suspended_author` / `resume_unsuspended_author` ← `workspaces.member_(un)suspended` | A's scheduled sends | no | unchanged |
| `bff.end_signed_out_sessions` ← `identity.signed_out_everywhere`, `bff.end_suspended_sessions` / `workspaces.announce_member_(un)suspended` ← `identity.account_(un)suspended`, `identity.notify_way_in_added` ← `identity.way_in_added` | end sessions, fan out, send a notice | yes | instance-wide events: a workspace may not append them (#207's list admits only `consent_revoked` and `way_in_removed`, which have no consumer), `load_event` refuses one logged in a workspace, and their kinds are now instance-only. Unchanged |

`Membership::workspace_live` is a provided method on the kernel's
`Membership` trait: `resolve` for a client bound to the workspace, which the
workspaces module answers with "is it live" (`deleted_at IS NULL`). The real
flows still act: removing a member deletes the member row in the transaction
that publishes `member_left`, and deleting a workspace sets `deleted_at` in
the one that publishes `workspace_deleting`, so by the time a delivery runs
the state matches. A disconnect deletes the connection row in the
transaction that publishes `connection_removed`.

Trade-off: someone removed and added back to a workspace before
`revoke_departed_consents` / `revoke_departed_grants` ran keeps the consents
and grants they had (a forgery and a quick re-add look the same to the
port). Deliveries run within seconds; an admin who re-adds someone that
fast meant them to stay.

## 3. The tick's clock

`tick` is split into `tick` (begin kernel, `tick_in`, commit) and
`tick_in(tx, schedule)`, so a test can stall a tick's transaction. The tick
number comes from `clock_timestamp()` (the statement's clock) instead of
`now()` (the transaction's start). A stalled tick then computes the current
tick, finds it recorded already (`last_tick < excluded` false, `last_tick >
excluded + 1` false) and enqueues nothing. Chosen over a `+ 2` margin, which
would keep the stale value and only widen the window, and would let a clock
jump of two periods go unhealed for longer.

## 4. What a workspace transaction reads of the instance-wide rows

### What it needed, checked on Postgres 17

- A plain `INSERT` checks only the `INSERT` policies' `WITH CHECK`.
- `INSERT ... RETURNING` also checks the new row against the `SELECT`
  policies' `USING`, and fails the statement ("new row violates row-level
  security policy") if no `SELECT` policy admits it.
- `INSERT ... ON CONFLICT` (`DO NOTHING` included, with or without a
  conflict) does the same: the new row must pass a `SELECT` policy.
- A policy on `xmin` ("rows of this transaction") cannot serve: the new row
  is checked before it has an `xmin`, and a row inserted under a savepoint
  carries the subtransaction's id. Refused.

So the policy served exactly two needs: the `RETURNING id` of
`EventBus::publish` for an instance-wide event appended from a workspace
(`identity.consent_revoked`, `identity.way_in_removed`), and an `ON
CONFLICT` enqueue of an instance-wide job from a workspace, which the kernel
has refused since #207 (`lift_for_instance_rows`, and `instance_rows_insert`
admits only event deliveries, which `publish` writes without `ON CONFLICT`
or `RETURNING`).

### The change

- `EventBus::publish` writes every instance-wide event (`workspace()` is
  `None`) with `append_instance_wide`: the id is a v7 UUID chosen in Rust,
  the event and its deliveries are one statement (a data-modifying CTE), and
  nothing is read back. Workspace events keep the `query_scalar!` statement
  as it was (same text, so the offline query cache is unchanged).
- Migration `20261021150000_kernel__instance_rows_unreadable_from_workspaces`
  drops `instance_rows_read` on both tables, in one transaction with a 5 s
  lock timeout.

| Table | Kernel scope | Workspace transaction | Neither |
|---|---|---|---|
| `kernel.events` | every row (`workspace_isolation`) | its own workspace's rows; adds a listed instance-wide event (`instance_rows_insert`); reads no instance-wide row | nothing (the kernel lifts it for its own insert) |
| `kernel.jobs` | every row | its own workspace's rows; adds a fresh delivery of a listed event to a listed consumer; reads no instance-wide row | nothing |
| `kernel.schedules` | every row (`kernel_scope_only`) | nothing | nothing |

The maintenance scope is not admitted by the kernel's tables, before or
after. The kernel scope's access is unchanged.

### Rollback SQL

sqlx has no down migrations and refuses to start with an applied migration
it does not know. **Before redeploying an image older than this change,
run this once as the migration login.** Checked on a database migrated to
`next` and one migrated to this change then rolled back: identical
`pg_policies` for schema `kernel` and identical `_sqlx_migrations`, and
migrating again applies cleanly.

```sql
-- Roll back campbooks-cloud#211's policy change BEFORE redeploying an image
-- older than it. Run once, as the migration login (the tables' owner).
-- Restores the exact `instance_rows_read` policies of 20261021010000 and
-- forgets this change's migration, so the older image starts and a later
-- upgrade applies it again.
BEGIN;
SET LOCAL lock_timeout = '5s';
CREATE POLICY instance_rows_read ON kernel.events FOR SELECT USING
    (workspace_id IS NULL
     AND nullif(current_setting('campbooks.workspace_id', true), '') IS NOT NULL);
CREATE POLICY instance_rows_read ON kernel.jobs FOR SELECT USING
    (workspace_id IS NULL
     AND nullif(current_setting('campbooks.workspace_id', true), '') IS NOT NULL);
DELETE FROM _sqlx_migrations WHERE version = 20261021150000;
COMMIT;
```

Restoring the policy under the new image is harmless (it reads nothing
through it); the image only adds the refusals above.

## 5. Rolling deploys

Two windows while old and new processes overlap: an old worker lacks
`load_event`'s workspace check (#207) and this change's instance-only
refusal until it is replaced (accepted, as in #207); and once the migration
ran, an old process publishing `identity.consent_revoked` or
`identity.way_in_removed` from a workspace (removing a workspace client, a
departed member's consents, a removed mailbox's last way in) is refused
(the `RETURNING` above) until it is replaced: an API request fails with a
500, a delivery retries with backoff and succeeds on a new worker.

## Prod risks

- **Background work that could stop.** None of the instance-only kinds is
  enqueued with a workspace by any code path, so no legitimate job is
  refused; a refused row is logged at WARN with its kind and kept as failed.
  The six re-checking consumers each make one more port call per delivery;
  a port error retries the delivery (10 attempts with backoff) rather than
  acting, so a long connectors or workspaces outage could leave a way in, a
  consent or a grant in place until the deliveries are retried from the
  admin. `identity.forget_connected_way_in` and `name_connected_mailbox` now
  only reach the port when a mailbox record matches, so deliveries for
  untracked ways in cost nothing more.
- **Event ids.** Instance-wide events now get their v7 id from the
  application's clock instead of the database's. Replay and pruning order
  by id and time; a few milliseconds of skew between hosts reorders nothing
  that matters (replay from an id is inclusive, deliveries are keyed by
  event id).

## Left open

- The consumers of `conversations.*` events in connectors
  (`connectors.queue_send` ← `send_requested`, `connectors.request_changes`
  ← `changes_requested`) act on the workspace's own connections at the
  provider (send mail, apply label changes) and take the event as the
  request; a forgery inside the workspace could make its own mailbox send.
  A follow-up issue: check the send or change request row in conversations
  before acting.
