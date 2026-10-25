# Tasks

## 1. connectors: provider and settings
- [x] 1.1 Migration `20261105010000_connectors__microsoft.sql`: `connections.provider` CHECK gains `microsoft`
- [x] 1.2 `Provider::Microsoft` and every exhaustive match (start_connection offered list, credentials refresh, intake, removal revoke = none, sync_stream idle arm, send_message `not_connected`, request_changes `unsupported`, calendar = not offered, tracked); `contracts/*` enums
- [x] 1.3 `MicrosoftConfig` (`CAMPBOOKS_CONNECTORS__MICROSOFT__ENABLED`, `…__TENANT`), checked only while on; `Deps::microsoft` offered only with the flag, the client and the secret key (warnings otherwise); boot tests with no new variable and with production's poll bounds

## 2. connectors: the Microsoft client
- [x] 2.1 `microsoft/`: token refresh at the tenant's endpoint, Graph `/me`, fixed hosts, 15 s timeouts, 64 KiB answers, no redirects
- [x] 2.2 Error mapping: `invalid_grant` / `interaction_required` / AADSTS codes → `InvalidGrant`; 429 and 503 + `Retry-After` → `RateLimited`; 401, 404, 5xx
- [x] 2.3 `/me` validation (id, address, display name) and scope normalisation; unit tests against wiremock

## 3. connectors: intake, health, discovery
- [x] 3.1 `GrantIntake::mailbox_scopes("microsoft")` while offered; `adopt` reads `/me`, keys the mailbox by the Graph id, refuses a different Graph id on a refresh, binds `address_verified` to the verified address
- [x] 3.2 The messages stream's run: token refresh, `needs_reauth` / `throttled`, then the longest interval; nothing read
- [x] 3.3 Discovery answers `use_provider: microsoft` while offered
- [x] 3.4 Front-door tests (connect, same account, reconnect with another account, another member, consent incomplete, flag off, client missing) and health tests

## 4. identity
- [x] 4.1 A second Microsoft client for the mail tenant, used by connect and reconnect round trips only
- [x] 4.2 A connect adds a way in only when sign-in's tenant admits the account
- [x] 4.3 Microsoft scopes compared without the Graph prefix, ignoring case; tests

## 5. Connect screen
- [x] 5.1 Route `use_provider: microsoft` to the provider step ("Connect Outlook", "Also use this account to sign in"); keep "Outlook is coming soon" for `not_supported_yet`
- [x] 5.2 i18n en, pt-PT, es-ES, fr
- [x] 5.3 Stories: coming soon, Connect Outlook, a play-less idle twin, 375px; client tests for the routing

## 6. Finish
- [x] 6.1 `deploy/README.md` (Entra app registration for mail, publisher verification) and `deploy/.env.example`
- [x] 6.2 OpenAPI regenerated; client types regenerated
- [x] 6.3 Tracking plan: `connectors.provider` gains `microsoft`; generated files regenerated
- [x] 6.4 Changelog entry `changelog/unreleased/added/315-microsoft-mail-connection.md`
- [x] 6.5 Gates green in CI; UI verified at 375px and desktop (recording)
