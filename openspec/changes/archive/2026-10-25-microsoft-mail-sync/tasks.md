# Tasks

## 1. Settings
- [x] 1.1 `CAMPBOOKS_CONNECTORS__MICROSOFT__{BACKFILL_DAYS,MAX_MESSAGE_BYTES,RUN_BUDGET_BYTES,REQUESTS_PER_MINUTE,MAX_FOLDERS,NOTIFICATIONS}`, read leniently and checked only while Microsoft mail is on
- [x] 1.2 The notification URL from `CAMPBOOKS_PUBLIC_URL` (`https`, not localhost), else polling with a warning; boot tests with production's poll bounds and no Microsoft variable

## 2. Graph client
- [x] 2.1 Folders (well-known by name, the tree), delta pages with tokens only, a message's state, `$value` with the size cap, subscriptions (create, renew)
- [x] 2.2 410 / `syncStateNotFound` / `resyncRequired` as an expired delta; `Retry-After` capped at an hour

## 3. Adapter
- [x] 3.1 `microsoft/state.rs`: roles, tokens, contract flags
- [x] 3.2 `microsoft/read.rs`: the cursor, one page per folder, new / changed / removed, look-ups, resync, byte budget
- [x] 3.3 `sync_stream`: the Microsoft run (budget, subscription, read, apply), replacing the idle arm

## 4. Notifications
- [x] 4.1 Migration `20261106010000_connectors__microsoft_sync.sql`: `connectors.microsoft_subscriptions`, RLS, maintenance `FOR SELECT`
- [x] 4.2 Subscribe / renew in the run; retry after a failure; the longest interval while live
- [x] 4.3 `POST /hooks/microsoft/mail`: validation handshake, bounded body, `clientState` check, 404, `run_now` per connection; mounted by the server
- [x] 4.4 `architecture.toml` maintenance-scope entry; the server's policy list

## 5. Tests
- [x] 5.1 Fake Graph in `testkit_microsoft.rs`: folders, delta paging (`nextLink` → `deltaLink`), 410, 429 `Retry-After`, `$value` over the limit, subscriptions
- [x] 5.2 Sync tests: backfill, incremental, moves and removals, resync, throttling, oversized, read-only
- [x] 5.3 Callback tests: handshake, wrong `clientState`, unknown subscription, a paused stream, renewal, no callback → polling
- [x] 5.4 RLS test for the new table

## 6. Security review
- [x] 6.1 The folder cap: well-known folders always read, folders read before kept ahead of new ones, `$orderby=displayName`, search folders left out; a folder is gone only to a complete listing; a well-known folder not named once keeps its role
- [x] 6.2 Graph's 5000-message `$filter` limit: first rounds newest first, counted; a truncated folder's unlisted messages are never removed, nor those of a folder not read while the listing is partial
- [x] 6.3 The callback rate limited (per process in memory, then per address and per instance), and mounted only while notifications are on
- [x] 6.4 The instance key's fingerprint kept with the `clientState` hash: another one makes a new subscription
- [x] 6.5 `…__MAX_MESSAGE_BYTES` at most 64 MiB (four downloads in flight); subscription calls charged to the connection's budget

## 7. Finish
- [x] 7.1 `deploy/README.md` (the notification URL, the proxy) and `deploy/.env.example`
- [x] 7.2 OpenAPI unchanged (no operation); client types unchanged
- [x] 7.3 Tracking plan unchanged (no event, no Coverage row)
- [x] 7.4 Changelog entry `changelog/unreleased/added/316-microsoft-mail-sync.md`
- [x] 7.5 Gates green in CI; no UI change (backend recording)
