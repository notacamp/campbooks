# Tasks

## 1. Kernel: recording and sequencing

- [ ] 1.1 Migration `kernel__changes` (`kernel.changes`, `kernel.change_heads`, indexes, grants to `campbooks_app`); `cargo xtask arch` passes
- [ ] 1.2 `Change { record, op }` and `DomainEvent::changes()` (default none); `EventBus::publish` writes declared changes and the workspace's shared lock and one coalesced `kernel.sequence_changes` job (skipped while a committed one is pending) in the publisher's transaction; verify rollback leaves nothing
- [ ] 1.3 `kernel.sequence_changes` (queue `changes`): exclusive workspace try-lock (busy: reschedule, no attempt spent), dense numbering of all unsequenced changes in publish order, one `NOTIFY kernel_changes`; verify out-of-order commits never produce a gap a reader can skip, a burst of 1 000 events takes a handful of jobs with gapless ordered cursors, producers are not delayed while a long producer blocks sequencing, and a re-run is a no-op
- [ ] 1.4 `kernel.prune_changes` (hourly, `CAMPBOOKS_CHANGES__RETENTION_HOURS`; re-enqueues sequencing for leftovers) and `changes::purge_workspace`; verify
- [ ] 1.5 `CAMPBOOKS_CHANGES__*` configuration with startup checks

## 2. Kernel: the stream

- [ ] 2.1 Hub: one `PgListener` per process, per-workspace `watch` wake-ups, reconnect, poll fallback
- [ ] 2.2 Stream: start/resume (`Last-Event-ID`, `after`), `ready`, `reset` (unknown, expired, too far behind), paging with pull backpressure, heartbeats, `closing` at max age or token expiry, per-principal limit (429), query semaphore
- [ ] 2.3 Visibility: scopes per record type from the operation catalogue; `Visibility` port (default: container-less only); membership re-check on own-member and workspace-delete changes → `revoked`

## 3. Server

- [ ] 3.1 Mount `GET /api/v1/w/{workspace_id}/changes` (`changes.stream`, `workspaces:read`, `x-campbooks`), infrastructure owner `changes` accepted by `cargo xtask arch`; start the hub in the `api` role
- [ ] 3.2 Verify it streams through `/bff/api/v1/w/{ws}/changes` (timeout and compression layers don't cut or buffer it)

## 4. Workspaces module

- [ ] 4.1 `workspaces.workspace_updated` v1, published by `workspaces.update` when something changed; `events.json` regenerated
- [ ] 4.2 `changes()` on the six events; purge job deletes the workspace's changes

## 5. Tests (server, `#[sqlx::test]` with an in-process test client)

- [ ] 5.1 End to end: rename / join / role change / leave → frames to a subscribed member
- [ ] 5.2 Resume after disconnect, ordering, `reset` for expired/unknown cursors
- [ ] 5.3 Access: non-member 404, anonymous 401, missing scope 403, scope-filtered changes advance the cursor, removed member gets `revoked`
- [ ] 5.4 Many concurrent subscribers each receive every change once; two hubs (simulated replicas) on one database agree on cursors
- [ ] 5.5 Heartbeat and `closing` with short configured intervals
- [ ] 5.6 No personal data: frame bodies have exactly the allowed keys, and no name/email/workspace name appears in any frame

## 6. Client

- [ ] 6.1 `records.gen.ts` generated from `openapi.json` (`api:generate`, `api:check`)
- [ ] 6.2 `ChangeSource` seam + web `EventSource` source; `subscribeToChanges` (cursor resume, backoff, hidden-tab pause, stop on 404/`revoked`); unit tests with a fake source and fake timers
- [ ] 6.3 Cache binding: `[operationId, params]` keys, batched invalidation, `delete` removes detail queries, `reset` invalidates the workspace; `modules/system` keys migrated

## 7. Finish

- [ ] 7.1 OpenAPI regenerated, client types regenerated, `.sqlx` regenerated
- [ ] 7.2 `docs/architecture.md` (feed section), self-hosting note on proxy buffering, CHANGELOG
- [ ] 7.3 Gates green (Rust, OpenSpec, client); no UI change
