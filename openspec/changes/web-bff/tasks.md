# Tasks

## 1. Kernel

- [x] 1.1 `TokenVerifier::verify_id_token` / `verify_logout_token` on the shared JWKS cache (issuer, audience = client id, `azp`, nonce, `iat`, `exp`, logout event, no nonce on logout tokens); verify with tests for every rejection
- [x] 1.2 `Registry::build_with` for background work owned outside modules; verify naming rules still apply
- [x] 1.3 `testing::TestIdp`: token endpoint (authorization code + PKCE S256, rotating refresh tokens, `client_secret_post`), revocation, `authorize`, ID and logout tokens

## 2. Session store

- [x] 2.1 Migration `bff__sessions` (schema `bff`: sessions, login transactions); `cargo xtask arch` treats `bff` as an infrastructure schema
- [x] 2.2 Token encryption (AES-256-GCM, HKDF from `CAMPBOOKS_SECRET_KEY`, row/column-bound AAD); verify tampering, wrong key and moved ciphertexts fail
- [x] 2.3 Lookup by token hash, idle/absolute timeouts, rotation with grace; verify with integration tests
- [x] 2.4 `bff.prune_sessions` every 15 min; verify it deletes only ended sessions and expired sign-ins

## 3. Endpoints

- [x] 3.1 `/bff/login` (PKCE, state, nonce, `return_to` validation, `SameSite=Lax` login cookie)
- [x] 3.2 `/bff/callback` (single-use state bound to the browser, `iss`, code exchange, ID token, new session, landing page instead of a redirect)
- [x] 3.3 `/bff/session` (never a token)
- [x] 3.4 `/bff/api/*` in-process with the bearer token; refresh shortly before expiry, single-flight across requests and processes; `session_required` without a session
- [x] 3.5 CSRF check on unsafe methods (`X-Campbooks-CSRF` + `Sec-Fetch-Site`/`Origin`)
- [x] 3.6 `/bff/logout` (revoke, clear, end-session URL without a token) and `/bff/backchannel-logout`
- [x] 3.7 OpenAPI: `session` cookie scheme, explicit security on every BFF operation; `cargo xtask arch` allows `session` only on `/bff/*` and without scopes

## 4. Configuration and deployment

- [x] 4.1 `CAMPBOOKS_BFF__*`, `CAMPBOOKS_SECRET_KEY[_FILE]`; startup check and one warning on plain-http cookies; BFF off (503) without issuer or client secret
- [x] 4.2 Compose: `server-init` + `server-secrets` volume; `idp-provision` writes the client secret there and registers the back-channel URL (https only); server mounts it read-only; `docker compose config -q`
- [x] 4.3 Request logs record the path, not the query string

## 5. Client

- [x] 5.1 Web transport: CSRF header; `session_required` → `/bff/login?return_to=<page>` once
- [x] 5.2 `useSession()` over `/bff/session`
- [x] 5.3 Vite dev proxy forwards `/bff` unchanged (no more fake passthrough)

## 6. Follow-ups (not in this change)

- [ ] 6.1 Sign-in / account UI in the web app once the design system lands; consume `useSession()`
- [ ] 6.2 Decide the logout UX with Rauthy (see `decisions-needed.md`)
- [ ] 6.3 Rate-limit `/bff/login` per client address
- [ ] 6.4 Previous-key support for `CAMPBOOKS_SECRET_KEY` once something durable is encrypted with it
- [ ] 6.5 A "your sessions" page (list and end sessions; the user-agent hint is stored for it)

## 7. Wrap-up

- [x] 7.1 OpenAPI regenerated (`cargo xtask openapi`), client types regenerated (`pnpm api:generate`), `.sqlx` regenerated
- [x] 7.2 End to end against the real Rauthy container (Chromium): sign-in, landing with the `Strict` cookie, `/bff/session`, `/bff/api`, refresh with concurrent requests (one refresh grant), CSRF rejection, logout with revocation; back-channel logout delivery checked up to Rauthy's https requirement
- [x] 7.3 CHANGELOG, `docs/architecture.md`, `AGENTS.md`, `deploy/README.md`, `deploy/.env.example`, `clients/README.md`
- [x] 7.4 Gates green: fmt, clippy, tests, `cargo xtask arch`, `cargo xtask openapi --check`, sqlx prepare check, `cargo deny`, `cargo machete`, client lint/typecheck/test/deps/deadcode/format/api/i18n/build, `openspec validate --all --strict`
- [x] 7.5 UI: no new screens; the visible change is the sign-in round trip through the IdP, recorded at 375px and desktop for the PR
