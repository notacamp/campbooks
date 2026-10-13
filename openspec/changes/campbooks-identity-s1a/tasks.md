# Tasks

## 1. Kernel

- [x] 1.1 `auth::KeySource` and `TokenVerifier::in_process`: an in-process issuer's JWKS, parsed and cached like a fetched one; tests for caching, a new `kid`, and a failing source
- [x] 1.2 `connections::GrantIntake` + `ProviderGrant` + `NoGrantIntake`; `AppState::with_grant_intake` and `with_auth`
- [x] 1.3 `crypto::derive_secret` (HKDF per purpose); test for stability and separation
- [x] 1.4 `IdToken` carries `email_verified` and `auth_time`; `TestIdp` sends `email_verified`, can withhold refresh tokens and exposes its JWKS

## 2. Identity module

- [x] 2.1 Migration `identity__init` (accounts, provider identities, sessions, authorize requests, upstream flows, pending sign-ups, held grants, refresh families and tokens, codes, signing keys, audit events); grants to the runtime role
- [x] 2.2 Signing keys: generate, seal, schedule on demand under an advisory lock, JWKS; tests for rotation with overlap, concurrent first starts, an unreadable key, sealing
- [x] 2.3 First-party clients in code; exact and loopback redirect matching; constant-time secret check
- [x] 2.4 Discovery, `/oauth2/jwks`, `/oauth2/authorize` (errors in RFC order, SSO, `prompt`, `max_age`), `/oauth2/token`, `/oauth2/revoke`, `/oauth2/end_session`
- [x] 2.5 Google round trip (state bound to the browser, nonce, PKCE, ID token checked), consent retry for a missing refresh token, pending sign-up, `/auth/new-account` with CSRF check
- [x] 2.6 Held grants and `identity.list_held_grants` / `adopt_held_grant` / `discard_held_grant`
- [x] 2.7 Events `identity.account_created`, `identity.signed_in`, `identity.refresh_reuse_detected`; audit trail; `identity.prune` every 15 minutes
- [x] 2.8 End-to-end tests against a fake Google (TestIdp): sign-up, sign-in, SSO, every protocol refusal, rotation and reuse, revocation, logout, native client, two people with one address, inactive issuer

## 3. Connectors and server

- [x] 3.1 `connectors` implements `GrantIntake` (mailbox scopes per provider; adopt resolves the member and calls `create_or_refresh_connection`); tests
- [x] 3.2 Server: `Composition.identity`, `with_composed_ports` (grant intake, in-process verifier), protocol routes and their OpenAPI owner in `cargo xtask arch`
- [x] 3.3 BFF: in-process issuer mode (derived secret, requests through the router); config tests; an end-to-end test from `/bff/login` to an adopted mailbox and logout, with no secret in any log line

## 4. Docs and gates

- [x] 4.1 `deploy/README.md` → Built-in sign-in; `.env.example`; `docs/architecture.md`; `docs/domain-map.md`; CHANGELOG
- [x] 4.2 OpenAPI and events regenerated, client types regenerated, gates green, terminal recording
