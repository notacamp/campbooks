# Design

## Context

The chain from #513 (campbooks-cloud#147):
`identity.account_suspended` / `identity.account_unsuspended` (instance-wide,
with `at`) → `workspaces.announce_member_suspended` / `…_unsuspended` (finds
the user by `sub`, publishes in each workspace) →
`workspaces.member_suspended` / `workspaces.member_unsuspended` →
`conversations.pause_suspended_author` / `…resume_unsuspended_author`, which
keep `conversations.member_suspensions (workspace_id, user_id, suspended,
as_of)` with the newest `at`. A rule whose author is suspended reads as
paused, `paused_reason = author_unavailable`.

Data ownership is unchanged: identity owns `identity.accounts`, workspaces
owns `workspaces.users`, conversations owns `conversations.member_suspensions`,
the kernel owns `kernel.jobs`. No module reads another's schema and no new
cross-module dependency or kernel-scope entry is added.

## 1. A counter, not the clock

`identity.accounts.suspension_seq bigint NOT NULL DEFAULT 0`. `suspend` and
`unsuspend` already lock the account's row (`accounts::lock`, `FOR UPDATE`)
and return `Unchanged` when nothing changes; the `UPDATE` that changes
`disabled_at` now also does `suspension_seq = suspension_seq + 1 RETURNING
suspension_seq`, so two changes to one account get consecutive numbers in
commit order, whatever the clock says. Backfill: 1 for an account suspended
today, 0 otherwise, so an odd count is a suspended account.

The events carry it as `seq: Option<i64>` (serde default, skipped when
absent): additive, the event versions stay 1. `at` stays on every event.

Ordering, the same everywhere (`workspaces::domain::suspension::Change::
is_older_than` and the conversations upsert):

| incoming | recorded | newer when |
| --- | --- | --- |
| `seq` | `seq` | incoming `seq` is higher |
| `seq` | none | always (the counter came later) |
| none | `seq` | never |
| none | none | incoming `at` is later (as before) |

Every event without `seq` was published before this change and every event
with one after, so "none is older" is exact. The workspaces fan-out treats
the same change again as not older and announces it again (a replay, or a
retry after a partial failure, repairs a delivery lost downstream);
conversations applies only a strictly newer change, so a repeat changes and
announces nothing.

## 2. The fan-out races

The two races of the issue's comment both come from the fan-out reading the
user's memberships while a membership changes:

- re-accepting at the moment of a suspension: the fan-out's read misses the
  new membership (fails open);
- being removed at the moment of a suspension: the fan-out publishes in the
  workspace, the departure is recorded, the later lift no longer reaches it
  (fails closed after a re-invite).

### Options

- **A. Derive the pause on joining from identity's current state.**
  workspaces cannot read `identity.accounts` (no cross-schema reads), so it
  would need a new synchronous port identity → workspaces, in the request
  path of accepting an invitation, and it still races: identity's state can
  change between the port's answer and the membership's commit.
- **B. Lock the user's row in the fan-out and in joining and leaving.** On
  its own a lock only orders the two; the joiner still has nothing to
  announce, since workspaces does not know the suspension.
- **C (chosen). B plus the newest change kept on the user's row.** The
  fan-out locks the user's row, records the change there (if not older),
  lists the memberships and publishes in each, and keeps the lock until the
  job commits. Joining (accepting, creating a workspace) and leaving lock the
  same row first. So:
  - the fan-out first: a joiner waits, then reads the recorded change and
    announces it in the workspace it joins; a leaver waits and leaves after
    the change was published there;
  - the join or departure first: the fan-out waits, then lists the
    memberships as they are after it.

  Every workspace the user belongs to therefore hears the newest change,
  from the fan-out or on joining. This is local to workspaces, needs no new
  port and holds whatever order the commits come in, so it is the more
  robust option.

Accepting an invitation announces the recorded change whether it is a
suspension or a lift: a lift that no longer reached a workspace the member
was removed from is told when they come back (finding 3). Creating a
workspace announces only a suspension (a new workspace holds nothing a lift
would undo). With nothing recorded (no change since this shipped), nothing
is announced.

conversations no longer rewrites a counted change on a departure
(`forget_member` touches only rows without `seq`): the recorded change is
the truth as of its count, and joining again announces the newest change,
which is never older. A departed author's rules are paused anyway
(`author_cannot_write`) while they are away.

### Locks and deadlocks

The lock is `SELECT … FROM workspaces.users WHERE … FOR NO KEY UPDATE`,
taken before any other lock of the transaction:

| path | transaction | order |
| --- | --- | --- |
| fan-out (`workspaces.announce_member_*`) | the job's (no workspace; `campbooks.user_id` set for the membership read) | user row; then each workspace's publish in its own `begin_for(workspace)` |
| `workspaces.accept_invitation` | `begin_for(workspace)` | user row → invitation row → membership insert |
| `workspaces.create_workspace` | `begin_for(new workspace)` | user row → signup code → workspace, membership |
| `workspaces.remove_member`, `workspaces.leave_workspace` (`departure::depart`) | `begin_for(workspace)` | user row → workspace row |

`FOR NO KEY UPDATE` conflicts only with itself and stronger locks, not with
the `KEY SHARE` locks a foreign key to the user takes (a security event
naming them, a membership row). So a transaction holding a workspace's row
lock and recording such a row never waits on it, and since the four paths
take the user's row first, none of them waits for a workspace lock while
another waits for its user row the other way round. The fan-out's
per-workspace publishes insert only into `kernel.events` / `kernel.jobs`,
which hold no lock any of these paths take.

### Isolation per table

| table | isolation | touched by |
| --- | --- | --- |
| `identity.accounts` | no row-level security (instance-level, identity's own); `suspension_seq` written only under the account's `FOR UPDATE` | `suspension::suspend` / `unsuspend` (admin API, a transaction setting no workspace) |
| `workspaces.users` | no row-level security (instance-level, only this module reads it); the three new columns written only under the row's `FOR NO KEY UPDATE` | the fan-out job, accept, create, depart |
| `workspaces.members` | unchanged (`workspace_isolation`, `users_see_their_memberships` for the fan-out's read as the user) | the fan-out's read |
| `conversations.member_suspensions` | unchanged (`workspace_isolation`, forced) | the two handlers and `forget_member`, in the delivery's workspace transaction |
| `kernel.jobs` | unchanged; read in the kernel scope (`begin_kernel`) | `jobs::admin::summary` |

## 3. Dead deliveries are seen

A generic kernel mechanism rather than kind names in the kernel (the kernel
knows no domain): `jobs::Watch { code, died }`, declared as `WATCH` on a
`Job`, `JobHandler`, `EventHandler` or `EventConsumer` (default `None`) and
carried by the registry (`Registry::watch`, `Registry::watched`). The
registry refuses a code that is not snake case, an empty line, or two kinds
sharing a code with different lines.

- When a job of a watched kind dies (out of attempts or fatal in
  `Worker::fail`, or its last lease expiring in `reap`), the worker logs
  `tracing::warn!(watch = code, kind, "{died}")` next to the existing
  `job failed for good` error. The line holds no id or content.
- `GET /admin/v1/system/jobs` (`system.admin_jobs`, `jobs.read`, unchanged
  scope) adds `watched: [{ code, kinds, failed }]`, one entry per watched
  code (0 when nothing died), computed from the counts it already reads.

The four suspension handlers are watched as `suspension_fanout`, logging
`suspension fan-out delivery died`; Admin → Jobs shows them as "Work to
watch: Suspensions reaching members' rules", with the dead count. An alert
in the hosted cloud's monitoring can match the line later; nothing in the
cloud repo changes here.

## 4. Keeping a paused author's proposal

`RuleView.paused_once_kept: Option<PausedReason>`: for a proposed rule, the
`paused_reason` it would have once confirmed (`Stored::paused_once_kept`:
the same rule as `active`), since confirming keeps its author. The rules
list shows, under a proposal with `author_unavailable`, "If kept, it stays
paused until whoever taught it is back or someone takes it over" (a neutral
status line, never the cause), and the toast after keeping says "Rule kept,
paused for now". In en, pt-PT, es-ES and fr. No new shortcut: Keep this rule
has none.

## Callers

| function | called by | runs in |
| --- | --- | --- |
| `identity::domain::suspension::suspend` / `unsuspend` | `identity.admin_suspend_account` / `identity.admin_unsuspend_account` (admin API, `users.suspend`) | the handler's transaction, no workspace (as before) |
| `workspaces::features::member_suspension::record` | the two fan-out handlers | the job's transaction (no workspace; `campbooks.user_id` set before the membership read) |
| `workspaces::features::member_suspension::publish` | the two fan-out handlers | `begin_for(workspace)`, one per workspace (as before) |
| `workspaces::domain::suspension::lock` | `record` (by subject), `accept_invitation::execute`, `create_workspace::execute`, `departure::depart` | the caller's transaction, first statement |
| `workspaces::domain::suspension::announce` | `publish`, `accept_invitation::execute`, `create_workspace::execute` | a transaction working in the workspace announced in |
| `conversations::features::suspended_authors::record` | the two conversations handlers | the delivery's workspace transaction (as before) |
| `conversations::features::forget_member` | `conversations.forget_member` | the delivery's workspace transaction (as before) |
| `Stored::paused_once_kept` | `rules::views` (list, get, create, confirm, update…) | the handler's `begin_for(workspace)` (as before) |
| `jobs::admin::summary` | `system.admin_jobs` | the kernel scope (`begin_kernel`, as before) |
| `Worker::died` | `Worker::fail`, `Worker::reap` | no database |

## Migration

Three migrations, one per module, after next's newest (`20261021080100`):
`20261021090000_identity__suspension_seq`,
`20261021090100_workspaces__member_suspension_state`,
`20261021090200_conversations__member_suspension_seq`. Each sets
`lock_timeout = '5s'`; the columns have constant defaults (no rewrite); the
only backfill is `UPDATE identity.accounts SET suspension_seq = 1 WHERE
disabled_at IS NOT NULL` (the suspended accounts). workspaces' new columns
start unknown (`suspension_at IS NULL`, a check keeps them consistent), and
existing `member_suspensions` rows have no `seq`.

### Rollback

Run as the database owner, then deploy the previous image. It restores the
previous shape exactly (checked against a database migrated to
`20261021080100`: same columns, defaults, constraints and
`_sqlx_migrations`). Events published meanwhile keep their `seq` field,
which the previous code ignores.

```sql
BEGIN;
SET LOCAL lock_timeout = '5s';
ALTER TABLE conversations.member_suspensions DROP COLUMN seq;
ALTER TABLE workspaces.users
    DROP CONSTRAINT users_suspension_known,
    DROP COLUMN suspended,
    DROP COLUMN suspension_seq,
    DROP COLUMN suspension_at;
ALTER TABLE identity.accounts DROP COLUMN suspension_seq;
DELETE FROM _sqlx_migrations WHERE version IN (20261021090000, 20261021090100, 20261021090200);
COMMIT;
```

## Risks

- A join or departure waits while a fan-out for that user runs (a few
  milliseconds per workspace). The fan-out holds the lock across its
  per-workspace commits; a database outage mid-way fails the job, which
  retries and announces again.
- A suspension still takes effect on rules a few seconds after it is made
  (two hops); a dead hop is now seen, not avoided.
- Accounts suspended before this ships have no recorded change in
  workspaces until their next suspension or lift; a suspended account cannot
  join anything meanwhile.
