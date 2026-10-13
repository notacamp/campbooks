# Proposal

## Why

ADR 0011 says clients hold a cache per workspace and follow a cursor-based
feed of the changes a member may see, catching up from their cursor after a
disconnect, instead of refetching lists on every screen. Nothing produces or
serves such a feed yet: the web app can only refetch on a timer
(`staleTime`), and nothing tells it when another member renames the
workspace or joins it.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/52.

## What Changes

- **Kernel: declared changes.** `DomainEvent` gains `fn changes(&self) ->
  Vec<Change>` (default: none). A `Change` is `{ record: RecordRef, op:
  upsert | delete }`, ids only. When `EventBus::publish` publishes an event
  that declares changes, it writes them to a new `kernel.changes` table in
  the same transaction, unsequenced, under the workspace's shared advisory
  lock, and enqueues `kernel.sequence_changes` unless one is already pending
  for the workspace, so a burst collapses into one job.
- **Kernel: the sequencer.** The `kernel.sequence_changes` job (queue
  `changes`) tries the workspace's exclusive advisory lock without waiting
  (busy: it reschedules itself a moment later, so producers never wait for
  it); holding it, no producer is in flight, and it gives all unsequenced changes of the
  workspace the next per-workspace cursors in publish order, commits, and sends
  `NOTIFY kernel_changes, '<workspace id>'`. Cursor order is therefore
  commit order per workspace, so a reader that has seen cursor N never
  finds a later-committed change below N (the gap ADR 0012 rejected offset
  consumers for).
- **Kernel: the stream.** `kernel::changes` serves one workspace's feed as
  Server-Sent Events: catch-up from a cursor (`Last-Event-ID` or `?after=`),
  then live, woken by one shared `LISTEN` per API process with a poll as
  safety net; pull-based paging (backpressure); heartbeats; a maximum
  connection time capped at the access token's expiry; a `reset` frame when
  the cursor is unknown, pruned or too far behind; changes filtered by what
  the caller may see (scopes of the operations that read each record type,
  and a container-visibility port for inbox/calendar/folder membership
  later). Frames carry `{cursor, record, op, version}` and nothing else.
- **Endpoint.** `GET /api/v1/w/{workspace_id}/changes`
  (`operationId` `changes.stream`, scope `workspaces:read`), served by the
  server as infrastructure owner `changes` (like `bff`). The web app
  reaches it unchanged through `GET /bff/api/v1/w/{workspace_id}/changes`:
  the BFF already streams the API's response body.
- **Workspaces module, first producer.** New event
  `workspaces.workspace_updated` (published by `workspaces.update`, ids and
  the names of the changed fields only). `workspace_created`,
  `workspace_updated`, `member_joined`, `member_left`,
  `member_role_changed` and `workspace_deleting` declare their changes on
  `workspaces.workspace` and `workspaces.member`.
- **Retention.** `kernel.prune_changes` (hourly) deletes changes older than
  `CAMPBOOKS_CHANGES__RETENTION_HOURS` (default 72); a client with an older
  cursor gets `reset`. A workspace's changes are deleted with it.
- **Client.** `lib/api/changes`: a `ChangeSource` seam (web: `EventSource`
  on the BFF; Tauri: a command bridge later, not built), a subscription
  that reconnects with the last cursor and pauses while the tab is hidden,
  and a cache binding that maps each change to TanStack Query invalidation
  through a generated `record type → reading operationIds` table
  (`records.gen.ts`, from the `x-campbooks.records` of non-mutating
  operations). Query keys follow one convention: `[operationId, params]`.
- **Configuration.** `CAMPBOOKS_CHANGES__HEARTBEAT_SECONDS` (25),
  `__MAX_CONNECTION_SECONDS` (900), `__POLL_INTERVAL_MS` (5000),
  `__MAX_BACKLOG` (10000), `__RETENTION_HOURS` (72),
  `__MAX_STREAMS_PER_PRINCIPAL` (8).

## Capabilities

### New Capabilities

- `change-feed`: how clients learn what changed in a workspace, resume after
  a disconnect, and keep their cache current without refetching everything.

### Modified Capabilities

None. The `workspaces` capability is still an open change
(`workspaces-tenancy`, not archived); the new event and the changes its
events declare are specified here, under `change-feed`, as its first
producer.

## Impact

- Code: `campbooks-kernel` (`events`, new `changes` module, config),
  `campbooks-server` (mounts the stream, infrastructure owner `changes`,
  shared listener), `campbooks-workspaces` (one new event, `changes()` on
  six events), `xtask` (`changes` as an infrastructure operation owner;
  `kernel.*` job names), `clients/app/src/lib/api` (+ generated
  `records.gen.ts`, query-key convention applied to `modules/system`).
- Data: migration `<ts>_kernel__changes.sql` (schema `kernel`:
  `changes`, `change_heads`; granted to `campbooks_app`, no RLS policy
  because, like the event log, it is read only through the kernel's API,
  which filters by workspace). No module schema changes.
- Events: one new event (`workspaces.workspace_updated` v1); listed in
  `openapi/events.json`.
- Dependencies: none new expected. axum's SSE support and
  `sqlx::postgres::PgListener` are already available; `tokio-stream` /
  `async-stream` are not needed (a hand-written `futures_util::stream::unfold`).
- Self-hosters: one more queue (`changes`), handled by the existing worker;
  nothing to run or configure. Reverse proxies must not buffer
  `text/event-stream` (documented; the response sets
  `X-Accel-Buffering: no` and `Cache-Control: no-cache`).

## Non-goals

- Record contents in frames, patching the cache from frames, and offline
  editing (ADR 0011 "later"); frames say *what* changed, clients refetch.
- The Tauri bridge (`changes_subscribe` command emitting webview events):
  the client seam is designed, not built (campbooks-cloud#17 owns native
  sign-in).
- Container-level visibility for inboxes, calendars and folders: the port
  and the `container` column exist; the modules that need them implement
  them.
- Changes for invitations and the security log (manager-only data): they
  wait for the container port's first real implementation.
- A cross-workspace "my workspaces" feed for the workspace switcher; a
  member's own workspace list is refetched on `reset`/404.
- Rebuilding the feed by event replay: the feed is short-lived by design
  (72 h); a lost feed is a `reset`.
- Any UI. No screen changes; the binding is exercised by tests.
