# Proposal

## Why

The job queue's unique and ordering keys span workspaces
(campbooks-cloud#209, found while building #207 and left open in the
archived change `2026-10-17-instance-rows-from-workspaces`). The unique
index is `(kind, unique_key)` over unfinished jobs, and a job waits for any
earlier unfinished job with its ordering key, whoever's it is. Most keys
embed an id (`connectors.sync_stream:<stream>`, `assistant.purge:<workspace>`,
`send:<send>`). A workspace that knows or guesses another workspace's key can
insert a row of its own with that key, never due (`run_at = 'infinity'`),
which the policies admit since it is its own row and no schedule key. The
other workspace's next enqueue of that key then finds a duplicate
(`ON CONFLICT DO NOTHING`) and is dropped, and any of its jobs ordered by
that key wait for ever. A `connectors.sync_stream:<stream>` row stops that
mailbox's sync for good. #207 only banned `schedule:%` keys on workspace
rows.

Modules touched: the kernel (`jobs`, `events`, the admin queue functions,
three migrations). Tests also in `connectors`. No new module, no new
cross-module dependency, no new endpoint, event or scope.

## What Changes

- **Unique keys are per workspace.** At most one unfinished job per kind,
  unique key and workspace, with the instance-wide jobs (no workspace) as one
  more group: they still dedupe among themselves, so a schedule's tick runs
  once per period and an account event's delivery once. A new unique index
  `(kind, unique_key, workspace_id) NULLS NOT DISTINCT` over unfinished jobs
  replaces `(kind, unique_key)`; every `ON CONFLICT` target names it
  (`jobs::enqueue*`, `EventBus::replay`).
- **Ordering keys are per workspace.** A job waits only for earlier
  unfinished jobs of its own workspace (or, instance-wide, of none) with its
  ordering key; the claim's check compares `workspace_key`, and a new index
  `(ordering_key, workspace_key, seq)` replaces `(ordering_key, seq)`.
  Enqueuers of a key are serialized per workspace: the advisory lock is
  taken on the workspace and the key together.
- **The admin's retry** treats a dead job's key as held only by an
  unfinished job of its own workspace, and `retry_kind` brings back one dead
  job per workspace and key.
- **Migrations** `20261021130000` and `20261021130100` build the two
  indexes `CONCURRENTLY` (one statement each, `-- no-transaction`, like the
  conversations' list indexes); `20261021130200` drops the old two in one
  transaction with a 5 s lock timeout.

## Non-goals

- Changing any caller's key: none relied on dedup or ordering across
  workspaces (the caller list is in `design.md`).
- The other two #207 leftovers in the issue: the payloads of
  `identity.consent_revoked` / `identity.way_in_removed` from a workspace
  are still not bound to it (neither has a consumer), and there is still no
  admin UI for discarding a schedule's run.
- `kernel.events` has no key-based dedup; nothing changes there.

## Impact

- A workspace can no longer stop or delay another workspace's keyed jobs.
- Within a workspace, and among instance-wide jobs, dedup and ordering are
  unchanged.
- An image from before this change cannot enqueue a job with a unique key
  once `20261021130200` ran (its conflict target names no index: 42P10).
  Upgrade or roll back the API and the worker together; the rollback SQL is
  in `design.md`.
