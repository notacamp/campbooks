# Tasks

## 1. Kernel

- [x] 1.1 `TokenVerifier::verify_id_token_with`: an ID token checked against a given issuer, returning extra claims; tests for a wrong issuer and extra claims
- [x] 1.2 `connections`: `ProviderGrant.extending`, `GrantIntake::reconnect_target`, the `ConsentDoor` port with `NoConsentDoor`; `AppState::with_consent_door`
- [x] 1.3 `TestIdp::set_id_token_claims` (extra or removed ID-token claims), to play Microsoft and Zoho

## 2. Identity

- [x] 2.1 Migration `identity__ways_in`: link tickets, flow columns; prune expired tickets
- [x] 2.2 One `Upstream` client for Google, Microsoft (tenant, `tid` issuer, `xms_edov`) and Zoho (accounts-server allowlist); settings incl. `TENANT`, `REDIRECT_PATH`, dev-only `BASE_URL`; offline access only with mailbox scopes
- [x] 2.3 `identity.start_link` (link/connect/reconnect, step-up, membership, mailbox scopes, reconnect target) and the `ConsentDoor` implementation
- [x] 2.4 `/auth/start?link=` (ticket + issuer-session match) and the link callback (link, refresh, refuse another account's, adopt with `extending`)
- [x] 2.5 `identity.list_ways_in`, `identity.remove_way_in` (step-up, `lock_and_count`, `last_way_in`); events `way_in_added`, `way_in_removed`
- [x] 2.6 Tests: every refusal path (stale step-up, foreign ticket, spent ticket, another account's identity, last way in, someone else's way in / connection), Microsoft issuer binding and `xms_edov`, Zoho allowlist before any request, offline only with mail

## 3. Connectors and server

- [x] 3.1 `connectors`: `reconnect_target`; `adopt` honours `extending`; `start_connection` delegates to the consent door; `complete_connection`, `authorizations` and the Google consent code removed; testkit connects through the intake
- [x] 3.2 Server: registers the consent door; rate limits on `/auth/start`, callbacks and `start_link`

## 4. Clients

- [x] 4.1 Settings → Ways to sign in (list, add, remove inline, reconnect, step-up, results), with the `sections` extension point rendering S3's passkeys
- [x] 4.2 Microsoft and Zoho on the sign-in screen
- [x] 4.3 Dev-only fake Microsoft and fake Zoho; recording scenario `ways-in`
- [x] 4.4 Stories for every state, tests, en/pt-PT/es-ES/fr

## 5. Docs and gates

- [x] 5.1 `deploy/README.md` (Microsoft, Zoho, redirect URIs, multi-DC), `.env.example`, CHANGELOG
- [x] 5.2 OpenAPI and events regenerated, client types regenerated, gates green, UI verified at 375px and desktop, recordings
