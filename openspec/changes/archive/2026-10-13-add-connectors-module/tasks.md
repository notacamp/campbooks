# Tasks

## 1. Kernel

- [x] 1.1 `kernel::crypto`: `SecretKey` (from `CAMPBOOKS_SECRET_KEY[_FILE]`), `Cipher::new(&key, purpose)`, `random_token`, `hash`, `same_hash`, `pkce_challenge`, moved from the BFF with their tests; the BFF uses it with its existing label (a test opens a value sealed by the old code path)
- [x] 1.2 Take #37's `JobHandler` / `EventConsumer` constructor injection (or merge it from `next` if #37 landed first)

## 2. Module `connectors`

- [x] 2.1 Scaffold (`cargo xtask new-module connectors`), scopes, config (`CAMPBOOKS_GOOGLE__*`, `CAMPBOOKS_CONNECTORS__*`), server composition with injected deps
- [x] 2.2 Migration: tables, indexes, RLS forced + policies on workspace-owned tables, instance-level `sync_due` and `provider_budgets`, grants
- [x] 2.3 Google OAuth client (authorize URL, token exchange with PKCE, refresh, revoke, id_token account), against wiremock
- [x] 2.4 Start and complete connection (binding, expiry, incremental consent, same-account upsert), events `connection_added`, `capability_granted` + account blob
- [x] 2.5 List, get, disconnect (revoke), sync now; visibility rules
- [x] 2.6 Health transitions and `connection_needs_reauth`
- [x] 2.7 Engine: `dispatch_due` schedule, `sync_stream` job (one page), adaptive intervals, provider budget, throttling and failure handling
- [x] 2.8 Gmail adapter: profile, labels cache, backfill pages, raw fetch + MIME parse (`mail-parser`) + blobs, label mapping, index
- [x] 2.9 Gmail incremental (`history.list`), label-only re-emit, removals, 404 → bounded resync
- [x] 2.10 Batches: build MessageChangeBatch v1, store, hold, publish `messages_changed` with cursor; prune schedule
- [x] 2.11 Consumers: `workspaces.member_left`, `workspaces.workspace_deleting` (revoke and delete at once)

## 3. Contracts

- [x] 3.1 `contracts/` from #37 carried unchanged; add `connectors.capability_granted.v1.schema.json` and `connection-account.v1.schema.json`
- [x] 3.2 Contract tests (`boon` 0.6.1) over every batch, account document and event the tests produce

## 4. Tests

- [x] 4.1 Fake Google (wiremock) + recorded fictional fixtures in en, pt-PT, es-ES, fr
- [x] 4.2 Flows: connect, wrong user, expiry, reconnect; backfill → incremental; label-only; deletion; 404 resync; 429; 401 → needs_reauth; disconnect revokes
- [x] 4.3 Scheduling with `FixedClock`: backoff, sync now collapse, fairness across workspaces
- [x] 4.4 RLS on every workspace-owned table; tokens never in responses, logs or payloads
- [x] 4.5 `#[ignore]`d smoke test against a real Google client (documented for staging; not run)

## 5. Finish

- [x] 5.1 OpenAPI and events catalogue regenerated; client types regenerated
- [x] 5.2 `deploy/README.md` → Connected accounts and `deploy/.env.example` for the Google client and redirect URI; CHANGELOG `[Unreleased]`
- [x] 5.3 Gates green (fmt, clippy, tests, arch, openapi, events, deny, machete, `.sqlx`, OpenSpec validate)

## 6. Follow-ups (not in this change)

- Outbound changes (`connectors.push_changes`) once `conversations.conversation_changed` carries an explicit intent
- Gmail push (`users.watch` + Pub/Sub, OIDC-verified), config-gated
- The `conversations` consumers of `messages_changed` and `capability_granted` (whichever of #36/#37 merges second)
- Connections screen and the web callback page (needs a prototype)
- A dedicated `sync` process role; secret key rotation with re-sealing
