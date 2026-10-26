# Tasks

## 1. Storage

- [x] 1.1 Migration `20261109010000_identity__instance_apps.sql`: kind `instance`, `post_logout_redirect_uris`, the widened checks, the CLI-only trigger
- [x] 1.2 `domain::clients`: `Kind::Instance`, the row's post-logout URIs, exact redirects, the ceiling in `may_ask_for`

## 2. The CLI

- [x] 2.1 `domain::instance_apps`: `validate` (exact canonical `https` URIs, loopback only in development, the ceiling, the look-alike name checks), `register`, `list`, `rotate_secret`, `remove`, each audited and tracked in its transaction
- [x] 2.2 `campbooks-server admin apps add|list|rotate-secret|remove`, on the migration login, with confirmation and the secret printed once

## 3. Protocol and consent

- [x] 3.1 `/oauth2/authorize`: the intersection of the requested scopes and an instance app's ceiling
- [x] 3.2 `/oauth2/end_session`: an instance app's hint and its post-logout redirect URIs
- [x] 3.3 The consent screen and Connected apps name an instance app as added by this server, in en, pt-PT, es-ES and fr (plus the schedule scopes' sentences); story and test

## 4. Tracking

- [x] 4.1 `tracking/patches/0016-instance-apps.yaml`: plan version 16, the three events, the kind `instance`, the widened client-id patterns
- [x] 4.2 `kernel::tracking::minted_client_id` and `domain::tracked` know `app_` ids

## 5. Tests

- [x] 5.1 CLI: create, list, rotate, remove; secret shown once and stored hashed; audit rows and tracked events
- [x] 5.2 Code + PKCE end to end with an instance app (Basic and post), the ID token's claims, refresh rotation and reuse
- [x] 5.3 A redirect URI it did not register refused; scopes above the ceiling not granted
- [x] 5.4 Consent asked once and remembered; revoked, asked again and its tokens stopped
- [x] 5.5 Removed: its refresh tokens refused at the token endpoint, its access tokens stopped
- [x] 5.6 RP-initiated logout back to its registered page only
- [x] 5.7 The runtime role cannot register, change or remove one, only record its last use

## 6. Last

- [x] 6.1 OpenAPI regenerated (descriptions of the client kinds), client types regenerated
- [x] 6.2 Tracking plan regenerated
- [x] 6.3 Changelog entry file (`changelog/unreleased/added/369-instance-apps.md`)
- [x] 6.4 Gates green in CI
- [ ] 6.5 Consent screen recorded at 375px and desktop, and the CLI in a terminal (the lead)
