# Tasks

## 1. Server

- [x] 1.1 Kernel: `analytics` config section (URL, site id, visitor key), off unless all three are set, startup check, key redacted
- [x] 1.2 Kernel: `crypto::hmac_sha256`
- [x] 1.3 System: `screen_views` slice: route template check, daily visitor id, tracking URL, `system.screen_view` resource
- [x] 1.4 System: `ScreenViewRelay` (bounded queue, one worker, drop on full or failure, `campbooks.screen_views` metric), composed in `Deps`
- [x] 1.5 OpenAPI regenerated, client types regenerated

## 2. Client

- [x] 2.1 `lib/telemetry/screen-views`: router template to `:name` route, Do Not Track and Global Privacy Control, fire and forget
- [x] 2.2 Wired to the router's `onResolved` in `app/providers`

## 3. Tests

- [x] 3.1 Server: templates pass; ids, UUIDs, numbers, query strings, fragments and titles are refused and never sent
- [x] 3.2 Server: visitor id stable within a day, different across days, accounts and keys
- [x] 3.3 Server: off means no network call; a full queue drops without waiting; Matomo failing is never the person's problem
- [x] 3.4 Client: route conversion, DNT and GPC send nothing, a failed send is dropped

## 4. Docs

- [x] 4.1 `deploy/README.md` and `deploy/.env.example`: settings, metric
- [x] 4.2 CHANGELOG
- [x] 4.3 Gates green in CI
