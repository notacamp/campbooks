# Design

## Context

- ADR 0011: one binary in roles (`api` serves HTTP, the BFF and the change
  feed; `worker` runs consumers and projections); read models are
  projections of the event log; clients follow a cursor-based SSE feed and
  catch up from their cursor.
- ADR 0012: events are written in the publisher's transaction and fanned out
  to one delivery job per consumer at publish time. Offset-based reading of
  the log was rejected because ids are assigned at insert, not commit: a
  reader can see event 11 commit before event 10 and skip 10 for good. A
  client cursor over the same log has exactly that problem.
- ADR 0013: every read operation lists the record types it returns in
  `x-campbooks.records`; `RecordRef { type, id }` names any record.
- #65: event payloads carry ids and non-personal facts only.
- The BFF runs `/bff/api/*` through the API router in-process and returns the
  API's response as is, body included, so a streaming body streams to the
  browser. Access tokens live 15 minutes and are refreshed by the BFF only
  when a request starts.

### Data ownership

Schema **`kernel`** (infrastructure, like the event log), migration
`<ts>_kernel__changes.sql`:

```sql
CREATE TABLE kernel.changes (
    id uuid PRIMARY KEY DEFAULT kernel.uuid_v7(),
    published bigint GENERATED ALWAYS AS IDENTITY,  -- insertion order
    workspace_id text NOT NULL,
    -- NULL until the sequencer assigns it; then dense per workspace.
    seq bigint,
    event_id uuid NOT NULL,           -- the event that declared it
    record_type text NOT NULL,        -- `<module>.<record>`
    record_id text NOT NULL,
    op text NOT NULL CHECK (op IN ('upsert', 'delete')),
    -- Visibility container (an inbox, a calendar…); NULL = every member.
    container_type text, container_id text,
    occurred_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, seq)
);
CREATE INDEX changes_unsequenced_idx ON kernel.changes (workspace_id, published) WHERE seq IS NULL;
CREATE INDEX changes_occurred_at_idx ON kernel.changes (occurred_at);
CREATE TABLE kernel.change_heads (
    workspace_id text PRIMARY KEY,
    head bigint NOT NULL          -- last assigned seq
);
```

Granted to `campbooks_app`; no RLS policy, like `kernel.events`: it is
read and written only through `kernel::changes`, which always filters by the
workspace the caller was admitted to. No module touches it.

### Events

- New: `workspaces.workspace_updated` v1 `{ workspace, fields: ["name" |
  "accent"], updated_by }`, published by `workspaces.update` in its
  transaction (only when something changed).
- Declared changes (`DomainEvent::changes`), ids only:

| Event | Changes |
| --- | --- |
| `workspace_created` | upsert `workspaces.workspace`, upsert `workspaces.member` (owner) |
| `workspace_updated` | upsert `workspaces.workspace` |
| `member_joined` | upsert `workspaces.member` (user) |
| `member_role_changed` | upsert `workspaces.member` (user) |
| `member_left` | delete `workspaces.member` (user) |
| `workspace_deleting` | delete `workspaces.workspace` |

`workspaces.member` ids are the user id (unique within its workspace).
The feed consumes no event through a handler; the sequencer is a kernel job.

### Scopes per endpoint

`GET /api/v1/w/{workspace_id}/changes` requires `workspaces:read` (the scope
for "see this workspace") through `Workspace<WorkspacesRead>`: 401 without a
token, 403 without the scope, 404 for a non-member, a foreign client token,
a deleted or unknown workspace. Beyond that, each change is delivered only if
the caller holds a scope of at least one non-mutating operation listing its
record type in `x-campbooks.records` (computed once from the composed
operation catalogue). A token with `workspaces:read` but no `mail:read`
never learns a mail record id. `operationId` `changes.stream`,
`x-campbooks`: query, `records: []`, not offered to automation or the
assistant. The web app reaches it as `GET /bff/api/v1/w/{ws}/changes`
(`bff.api_get`, session cookie, no CSRF header needed for GET).

## Decisions

### 1. Where cursors come from: a sequenced projection table

Producers already publish events in their transaction. An event type
declares the records it changed (`changes()`), so modules write no second
call and cannot forget it. When an event declares changes, `publish`, in the
publisher's transaction:

1. takes the workspace's **shared** advisory lock
   (`pg_advisory_xact_lock_shared(hash('kernel.changes:' || ws))`), held
   until the publisher commits; producers never block each other;
2. inserts the changes unsequenced (`seq` NULL);
3. enqueues `kernel.sequence_changes` for the workspace with
   `unique_key = <workspace>`: a burst of any size collapses into one
   unfinished sequencing job per workspace.

The sequencer job, in its transaction:

1. takes the workspace's **exclusive** advisory lock (waiting at most
   `lock_timeout` = 2 s, else it retries shortly): it now runs after every
   producer that is in flight has committed or rolled back;
2. numbers **all** unsequenced changes of the workspace from `head + 1`, in
   publish order (`event_id`, then `id`, both UUIDv7), and moves
   `kernel.change_heads.head`;
3. sends one `NOTIFY kernel_changes, '<workspace>'` and commits (its lock is
   released with the job marked succeeded, in the same commit).

Why the lock pair: the queue's unique key covers pending *and running*
jobs, so a producer can find the key taken by a sequencer that is already
running and has already numbered what it could see. Without the lock, that
producer's changes would wait, unnumbered, for the next burst. With it, a
producer that saw a running (or pending) job holds the shared lock until it
commits, so that job cannot number anything before the producer's changes
are visible; a producer that arrives after the sequencer took the lock
waits for its (millisecond) commit and then finds no unfinished job and
enqueues a new one. Cursor order is therefore commit order per workspace:
a reader that has seen cursor N never finds a later-committed change below
N. A waiting exclusive request queues ahead of new shared requests, so a
busy workspace cannot starve its sequencer; the price is that new producers
of that workspace wait for the in-flight ones while a sequencer waits (the
2 s `lock_timeout` bounds it). An hourly sweep re-enqueues sequencing for
any workspace left with unnumbered changes (a sequencer that ran out of
attempts). Delivery is at least once; the job is database-only, so it takes
effect once. Queue `changes` keeps it clear of sync and AI backlogs.

### 2. Waking streams: one listener per process

Each `api` process holds one `PgListener` on `kernel_changes` (not one per
connection, which would exhaust the pool) and a hub of per-workspace
`tokio::sync::watch` channels. A notification bumps its workspace's watch;
streams of that workspace wake and query from their own cursor. Notifications
can be lost (listener reconnecting), so each idle stream also polls every
`POLL_INTERVAL_MS` (5 s). Correctness never depends on `NOTIFY`; latency does.
Replicas share nothing but Postgres: a cursor issued by one replica is
valid on every other and after restarts, because it is a row in
`kernel.changes`.

### 3. The stream

```
GET /api/v1/w/{ws}/changes        Accept: text/event-stream
    Last-Event-ID: 41   (or ?after=41; header wins)

retry: 3000
event: ready         data: {"cursor":"41","head":"44"}
id: 42  event: change  data: {"cursor":"42","record":{"type":"workspaces.member","id":"…"},"op":"upsert","version":"<event id>"}
…
: heartbeat                                   (every 25 s)
event: reset         data: {"cursor":"44","reason":"cursor_expired"}
event: closing       data: {"reason":"max_age"}   then the body ends
```

- **Start**: no cursor → start at the head (the client has just fetched what
  it shows). A cursor above the head, below the oldest retained change, or
  more than `MAX_BACKLOG` behind → one `reset` frame carrying the head
  (`unknown_cursor` | `cursor_expired` | `too_far_behind`), then live from
  there. `reset` means "invalidate everything of this workspace".
- **Paging and backpressure**: changes are read 200 at a time and the next
  page is read only when the response body is polled again, so a slow client
  holds at most one page in memory and no database connection between pages.
  Several changes of the same record inside a page collapse to the last
  one (a refetch shows the current state either way).
- **Visibility**: filtered per change before framing (scopes, then the
  container port). Filtered changes still advance the cursor: the `id` of the
  next delivered frame, or the head carried by a heartbeat event every
  heartbeat (`event: heartbeat data: {"cursor":"…"}` when the cursor moved
  silently), so a client never re-reads what it may not see.
- **Membership can end mid-stream**: when the stream delivers a
  `workspaces.member` change about the caller, or the workspace's delete, it
  resolves membership again; if gone, it sends `event: revoked` and ends.
  Reconnecting then gets 404.
- **Max connection time** `MAX_CONNECTION_SECONDS` (900), shortened to the
  access token's `exp`: the stream sends `closing` and ends; `EventSource`
  reconnects with `Last-Event-ID`, which re-checks the token, scopes and
  membership (and lets the BFF refresh the token). A small random jitter
  spreads reconnects.
- **Limits**: at most `MAX_STREAMS_PER_PRINCIPAL` (8) open streams per
  subject per process (429 `too_many_streams`); a semaphore bounds
  concurrent feed queries per process so a wake of 10 000 streams cannot
  take the whole pool.
- **Frames carry no record contents and no personal data**: cursor,
  `RecordRef`, op, version. `version` is the id of the event that produced
  the change (UUIDv7, time-ordered); modules with row versions may supply
  their own later.
- **Middleware**: the request timeout covers the response head only; the
  compression layer already skips `text/event-stream`. Responses set
  `Cache-Control: no-cache` and `X-Accel-Buffering: no`.

### 4. Access-filtering hook

`kernel::changes::Visibility` is a port like `Membership`: given the
caller's `Access` and a change's container, may they see it? The default
admits every container-less change and refuses any container (fail closed).
Modules that introduce containers (inbox membership, shared calendars,
folders) implement it and the server registers it; the stream asks once per
container per connection and caches the answer until a change to the
container's own membership record arrives.

### 5. Client

- `ChangeSource` (the seam): `open({ workspace, after, onFrame, onError })
  → close()`. `createWebChangeSource` uses `EventSource` on
  `/bff/api/v1/w/{ws}/changes?after=…` (same-origin cookie). A Tauri source
  will call a Rust command that holds the stream with the keychain token and
  emits webview events: not built here.
- `subscribeToChanges(queryClient, workspace, source)`: keeps the last
  cursor, closes the stream 30 s after the tab becomes hidden and reopens
  from the cursor when it is visible again, reconnects with backoff after
  errors, stops on 404/`revoked` (and invalidates `workspaces.list`).
- **Mapping by convention**: query keys are `[operationId, params]`
  (e.g. `["workspaces.list_members", { workspace_id }]`). A generated
  `records.gen.ts` maps each record type to the operationIds of the
  non-mutating operations that list it in `x-campbooks.records`
  (`pnpm api:generate`, checked by `api:check`). A change invalidates the
  queries whose key starts with one of those operationIds and whose
  `workspace_id` is the stream's; a `delete` also removes detail queries
  keyed by that record id. Invalidations are batched per 50 ms.
  `reset` invalidates every query of the workspace.
- `modules/system` query keys move to the convention.

### Copying the old Rails app?

No. The Rails app pushed rendered HTML per user over Action Cable
(Turbo Streams on `people_<user_id>`): fire-and-forget, no cursor, contents
in the frame. A missed broadcast was lost until a reload. This design keeps
only the idea "the server tells the client"; it wins on durability (resume
by cursor), privacy (no contents) and the BFF fit (plain HTTP).

## Alternatives considered

### Transport: SSE vs WebSocket vs long-poll

- **SSE (chosen).** One-way is all the feed needs. Plain HTTP GET: goes
  through the in-process BFF unchanged, through proxies, HTTP/2 multiplexes
  it; `EventSource` reconnects by itself and sends `Last-Event-ID`. Costs: an
  open connection per tab (bounded by the per-principal limit), proxies that
  buffer must be told not to.
- **WebSocket.** Bidirectional, which we don't need until offline editing,
  and that can still be ordinary HTTP writes. An upgrade cannot pass
  through the BFF's in-process `oneshot`; we would need our own resume,
  heartbeat and auth-expiry protocol. Rejected.
- **Long-poll.** Simplest server, fits a request/response native bridge.
  But a request per batch and latency of a round trip after each, and the
  same cursor logic anyway. The SSE handler reads the same cursor pages, so a
  JSON long-poll mode can be added on the same endpoint later if a client
  needs one. Not built now.

### Source of changes

- **Tail the event log by id.** No new table, but the commit-order gap
  (ADR 0012) makes cursors skip changes; events would need typed decoding at
  read time to know what record changed; there is no per-workspace index;
  and #65's retention and per-workspace purge would remove resume points
  from under clients. Rejected.
- **One sequencing job per changing event** (the first draft). Simple and
  race-free without locks, but a burst of 1 000 events is 1 000 jobs that
  each find nothing to do but the first. Replaced by one coalesced job per
  workspace (`unique_key = <workspace>`) plus the shared/exclusive lock
  pair, which closes the window a running job leaves open (decision 1).
- **Projection table written by an event handler.** Faithful to "projections
  consume the log" and rebuildable by replay, but it needs a consumer per
  event type (registered by every publishing module) and still needs a
  per-workspace sequencing step to be gap-free. Replay is of little value
  for a 72-hour feed. The chosen design is this option with the changes
  captured at publish time instead of re-derived in a handler.
- **Producer-side sequencing** (each producer bumps `change_heads` in its own
  transaction). No job and no lag, but it serializes every writing
  transaction of a workspace on one row until it commits: mail sync would
  queue behind itself. Rejected.
- **Visibility by transaction ids** (`pg_current_xact_id()` per row; read
  only below the snapshot's `xmin`). No job and no lag, but any long
  transaction anywhere in the database stalls every feed of every workspace.
  Rejected; a possible optimization later if queue latency matters.
- **`LISTEN/NOTIFY` as the source.** Lossy, 8 KB payloads, not durable.
  Used for wake-ups only.

### Per-workspace vs per-member streams

- **Per workspace, filtered per subscriber at read time (chosen).** One write
  per change whatever the number of members; access is decided with the
  caller's current scopes and membership.
- **Per member (fan out a row per member who may see it).** Writes multiply
  by members, every membership or container change must rewrite history, and
  a role change would leave stale rows. Rejected.

### Frames: contents vs invalidation-only

- **Invalidation-only (chosen).** No personal data in the feed (#65), no
  second serialization of every record (the API's responses stay the only
  shape), refetches go through the normal endpoints so access checks and RLS
  apply exactly once, and out-of-order or duplicate frames are harmless
  (the refetch returns the current state). Cost: a round trip per affected
  query, bounded by batching.
- **Record contents.** Saves the refetch and is what offline editing will
  want, but every frame becomes personal data, needs per-field access rules
  and versioned shapes, and duplicates the API's schemas. Deferred to the
  offline-editing work, on this same feed.

## Risks / Trade-offs

- Feed lag = queue latency (ms normally; longer if every worker is busy and
  the `changes` queue waits). Screens that need read-your-own-writes
  already have the mutation's response.
- One connection per open tab per workspace; a member with many tabs hits
  the per-principal limit (8) and the oldest tabs fall back to `staleTime`.
- A session ended by back-channel logout keeps its stream until the access
  token expires (≤ 15 min), never longer.
- Retention of 72 h: a laptop closed over a long weekend gets `reset` and
  refetches what it shows; nothing is lost.
- The cursor is per workspace and opaque to clients (a decimal string);
  a database restored from backup is detected (cursor above head) and reset.
