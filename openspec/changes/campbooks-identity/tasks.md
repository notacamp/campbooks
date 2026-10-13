# Tasks

Gated: nothing below starts until the design is approved. Each numbered
group is one slice and one PR (see design → Slicing plan).

## 1. S1a — Issuer core, Google sign-in, BFF on the in-process issuer

- [ ] 1.1 Scaffold `crates/modules/identity` (`cargo xtask new-module identity`); migration `identity__init` (accounts, provider_identities, sessions, upstream_flows, authorize_requests, held_grants, clients with forced RLS + the `authorization_server` select policy, authorization_codes, refresh_families, refresh_tokens, signing_keys, audit_events); grants to `campbooks_app`; verify with the server's RLS and runtime-role tests
- [ ] 1.2 `xtask arch`: `/oauth2/*`, `/.well-known/*`, `/auth/*` are `identity` routes outside `/api/v1`, with `security(())`; verify with arch unit tests
- [ ] 1.3 Signing keys: generate Ed25519 (`aws-lc-rs`), seal with `kernel::crypto::Cipher`, bootstrap under an advisory lock, `identity.rotate_signing_keys` hourly (publish 7 d ahead, active 30 d, retire 2 d); verify with a `FixedClock` test of a full rotation and of many concurrent starts creating one key
- [ ] 1.4 Kernel: `TokenVerifier` `KeySource` (remote JWKS | local), shared parse/refresh path; `identity` registers the local source; verify the existing auth tests still pass and a token signed by `identity` validates through the local source and through `/oauth2/jwks`
- [ ] 1.5 Discovery (`openid-configuration`, `oauth-authorization-server`) and `/oauth2/jwks`; verify against RFC 8414 / OIDC Discovery required fields and `code_challenge_methods_supported = ["S256"]`
- [ ] 1.6 `/oauth2/authorize` for first-party clients: parameter validation order (client → redirect URI → rest), PKCE S256 only, stored request, 303 to `/sign-in`, `iss` in responses (RFC 9207); verify each RFC 6749 §4.1.2.1 / OAuth 2.1 error case
- [ ] 1.7 `/oauth2/token`: authorization_code (single use, replay revokes), refresh_token (rotation, reuse revokes family, no grace), client auth for confidential clients; RFC 9068 access tokens, ID tokens; verify each case
- [ ] 1.8 `/oauth2/revoke` (RFC 7009) and `/oauth2/end_session`; issuer sessions (`__Host-campbooks_auth`, idle/absolute); verify
- [ ] 1.9 First-party clients by migration: `campbooks-web` (secret derived from `CAMPBOOKS_SECRET_KEY`), `campbooks-native` (public); verify derivation is stable and never logged
- [ ] 1.10 Upstream RP core + Google: `/auth/start/google`, `/auth/callback/google` (state bound to browser, nonce, PKCE, ID token via kernel JWKS cache, `prompt=consent` retry when a new account got no refresh token); verify against a wiremock Google
- [ ] 1.11 Sign-in vs new account (`pending_account` → `POST /auth/new-account`), held grant creation, `identity.adopt_held_grant` / `discard_held_grant`, `identity.expire_held_grants`; verify every branch
- [ ] 1.12 Kernel `connections::GrantIntake` + `ProviderGrant`; `connectors` implements it over `create_or_refresh_connection`; server registers it; verify adoption creates the connection and a second adoption refreshes it
- [ ] 1.13 BFF: issuer defaults to `CAMPBOOKS_PUBLIC_URL`, token/end-session calls through the in-process router, derived client secret; verify the existing BFF test suite against the real issuer as well as `TestIdp`
- [ ] 1.14 Rate limits (Postgres-backed) on start/callback/token/new-account; audit events; telemetry privacy test covers identity routes
- [ ] 1.15 OpenAPI regenerated, client types regenerated, CHANGELOG updated, gates green, vhs recording

## 2. S1b — Sign-in screens

- [ ] 2.1 `clients/app/src/modules/identity`: `/sign-in` (providers, passkey placeholder hidden until S3), `/sign-in/new`; shortcuts registered; i18n en/pt-PT/es-ES/fr
- [ ] 2.2 First run (#451): "connect this mailbox here" step, Home offer for a held grant
- [ ] 2.3 Stories + tests; client gates green; UI verified at 375px and desktop; recording

## 3. S2 — Ways in, Microsoft, Zoho

- [ ] 3.1 `identity.start_link` with `link` / `connect` / `reconnect` purposes; refuse `way_in_belongs_to_another_account`; step-up (`max_age`)
- [ ] 3.2 `identity.remove_way_in` with `last_way_in` under a row lock; `list_ways_in`, `list_sessions`, `end_session`
- [ ] 3.3 Microsoft (`common`, `tid`-bound issuer check, `xms_edov`), Zoho (accounts-server allowlist, per-DC token exchange); verify with wiremock fakes incl. a forged `accounts-server`
- [ ] 3.4 `connectors`: consent endpoints delegate to `identity` (or are removed); callback page removed
- [ ] 3.5 Settings → Ways to sign in (list, add, rename, remove, sessions)
- [ ] 3.6 OpenAPI regenerated, client types regenerated, CHANGELOG updated, gates green, UI verified at 375px and desktop, recording

## 4. S3 — Passkeys

- [ ] 4.1 Add `coset` (exact version ≥ 2 weeks old, `cargo deny check`); migration `identity__passkeys`, `webauthn_challenges`
- [ ] 4.2 Registration and assertion verification (WebAuthn L3 §7.1/§7.2; EdDSA, ES256, RS256 via `aws-lc-rs`; attestation `none`); verify with spec test vectors and recorded browser responses, every rejection named
- [ ] 4.3 Passkey sign-in (discoverable, conditional UI) and passkey-only sign-up
- [ ] 4.4 Screens: "Use a passkey", Add passkey in Ways to sign in; i18n
- [ ] 4.5 OpenAPI regenerated, client types regenerated, CHANGELOG updated, gates green, UI verified at 375px and desktop, recording

## 5. S4 — Native, third-party clients, consent, MCP

- [ ] 5.1 Native: loopback any-port matching, private-use scheme; verify RFC 8252 cases
- [ ] 5.2 Workspace clients API (`identity:clients`, owner/admin): register, rotate secret (shown once), remove; client_credentials with `workspace_id`
- [ ] 5.3 Consent: `/consent` screen, `POST /auth/consent`, remembered and revocable consents; workspace-bound tokens; `Workspace<S>` checks the claim on user tokens too
- [ ] 5.4 RFC 8707 `resource` (`invalid_target`); RFC 9728 metadata; `WWW-Authenticate` `resource_metadata` + `scope` on 401/403
- [ ] 5.5 DCR (`/oauth2/register`, public clients only, rate-limited, unused-client expiry); Client ID Metadata Documents (SSRF guard, cache, exact `client_id`)
- [ ] 5.6 Settings → API access and Connected apps screens; i18n
- [ ] 5.7 OpenAPI regenerated, client types regenerated, CHANGELOG updated, gates green, UI verified at 375px and desktop, recording

## 6. S5 — Remove Rauthy

- [ ] 6.1 `deploy/compose.yaml`: drop `idp`, `idp-init`, `idp-provision`, `idp-data`; server env simplified; `docker compose config -q`
- [ ] 6.2 Delete `deploy/idp/`, `xtask/tests/idp_provision.rs`; settings rename `CAMPBOOKS_IDP_<P>_*` → `CAMPBOOKS_<P>__*` with a one-release fallback and warning
- [ ] 6.3 `deploy/README.md`, `.env.example`, `docs/architecture.md`, `docs/domain-map.md`, `openspec/config.yaml` context line; withdraw `openspec/changes/upstream-sign-in`; resolve the Rauthy entries in `decisions-needed.md`; ADR 0014 → Accepted
- [ ] 6.4 `workspaces` migration deleting users with no membership
- [ ] 6.5 CHANGELOG (what self-hosters see), gates green, recording

## 7. Before production

- [ ] 7.1 OpenID Foundation conformance suite (basic OP + config profiles, PKCE) against staging; results attached to #57
- [ ] 7.2 Security review #57 signed off
