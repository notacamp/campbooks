# Tasks

## 1. Settings
- [ ] 1.1 `CAMPBOOKS_CONNECTORS__MICROSOFT__{BACKFILL_DAYS,MAX_MESSAGE_BYTES,RUN_BUDGET_BYTES,REQUESTS_PER_MINUTE,MAX_FOLDERS,NOTIFICATIONS}`, read leniently and checked only while Microsoft mail is on
- [ ] 1.2 The notification URL from `CAMPBOOKS_PUBLIC_URL` (`https`, not localhost), else polling with a warning; boot tests with production's poll bounds and no Microsoft variable

## 2. Graph client
- [ ] 2.1 Folders (well-known by name, the tree), delta pages with tokens only, a message's state, `$value` with the size cap, subscriptions (create, renew)
- [ ] 2.2 410 / `syncStateNotFound` / `resyncRequired` as an expired delta; `Retry-After` capped at an hour

## 3. Adapter
- [ ] 3.1 `microsoft/state.rs`: roles, tokens, contract flags
- [ ] 3.2 `microsoft/read.rs`: the cursor, one page per folder, new / changed / removed, look-ups, resync, byte budget
- [ ] 3.3 `sync_stream`: the Microsoft run (budget, subscription, read, apply), replacing the idle arm

## 4. Notifications
- [ ] 4.1 Migration `20261106010000_connectors__microsoft_sync.sql`: `connectors.microsoft_subscriptions`, RLS, maintenance `FOR SELECT`
- [ ] 4.2 Subscribe / renew in the run; retry after a failure; the longest interval while live
- [ ] 4.3 `POST /hooks/microsoft/mail`: validation handshake, bounded body, `clientState` check, 404, `run_now` per connection; mounted by the server
- [ ] 4.4 `architecture.toml` maintenance-scope entry; the server's policy list

## 5. Tests
- [ ] 5.1 Fake Graph in `testkit_microsoft.rs`: folders, delta paging (`nextLink` → `deltaLink`), 410, 429 `Retry-After`, `$value` over the limit, subscriptions
- [ ] 5.2 Sync tests: backfill, incremental, moves and removals, resync, throttling, oversized, read-only
- [ ] 5.3 Callback tests: handshake, wrong `clientState`, unknown subscription, a paused stream, renewal, no callback → polling
- [ ] 5.4 RLS test for the new table

## 6. Finish
- [ ] 6.1 `deploy/README.md` (the notification URL, the proxy) and `deploy/.env.example`
- [ ] 6.2 OpenAPI unchanged (no operation); client types unchanged
- [ ] 6.3 Tracking plan unchanged (no event, no Coverage row)
- [ ] 6.4 Changelog entry `changelog/unreleased/added/316-microsoft-mail-sync.md`
- [ ] 6.5 Gates green in CI; no UI change (backend recording)
