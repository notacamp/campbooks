# Tasks

## 1. Configuration

- [x] 1.1 Add `AuthConfig` (`CAMPBOOKS_AUTH__ISSUER`, `__AUDIENCE`, `__JWKS_URL`, `__LEEWAY_SECONDS`) to the kernel `Config` and the test config; verify defaults, parsing, blank-as-unset and the startup check with config unit tests
- [x] 1.2 Check the auth settings at startup and log one warning when no issuer is set; document the settings in `deploy/.env.example` and `deploy/README.md`

## 2. Token validation

- [x] 2.1 Add `jsonwebtoken` (aws-lc-rs backend, no PEM) and confirm `cargo deny check` passes
- [x] 2.2 `TokenVerifier`: alg allowlist, `typ`, `kid`, key/alg match, `iss`, `aud`, `exp`, `nbf`/`iat` with leeway; verify with unit tests of claim mapping and end-to-end tests for each rejection
- [x] 2.3 JWKS cache: discovery, TTL from `max-age` (1 h default, 24 h cap), unknown-`kid` refresh with 30 s cooldown, single-flight, keep last good keys, 503 when never fetched; verify with tests against `TestIdp` (rotation, cooldown, TTL, outage, discovery, unreachable IdP)
- [x] 2.4 `Principal` with `subject`, `kind`, `client_id`, `scopes`, `workspace`; verify `client_credentials` (no `sub`) and `workspace_id` at the root and under `custom`

## 3. Scopes and the extractor

- [x] 3.1 `ScopeName` + `ScopeInfo`; `Module.scopes`; `system` declares `system:read`; `scope_catalogue()` and `campbooks-server scopes`; verify the catalogue is sorted, unique and module-prefixed
- [x] 3.2 `Scoped<S>` extractor and the 401/403 problem+json with RFC 6750 challenges; verify missing, malformed, expired, unconfigured and insufficient-scope responses end to end

## 4. OpenAPI and architecture rules

- [x] 4.1 `bearer` security scheme listing the catalogue; health and version marked `security(())`; verify in server tests
- [x] 4.2 `cargo xtask arch`: fail on operations without explicit `security`, unknown schemes or scopes, scope-less bearer requirements, and badly named scopes; verify with xtask unit tests and by removing `security(())` from a real handler
- [x] 4.3 Server test that every operation documented as scoped answers 401 without a token

## 5. Follow-ups (not in this change)

- [x] 5.1 Once notacamp/campbooks#431 merges, make `deploy/idp/provision.sh` register the scopes from `campbooks-server scopes` instead of a hand-written list, and set `CAMPBOOKS_AUTH__ISSUER` / `CAMPBOOKS_AUTH__JWKS_URL` in `deploy/compose.yaml` — obsolete: Campbooks issues its own tokens (ADR 0014, `2026-10-14-remove-rauthy`); `deploy/idp/` is gone
- [x] 5.2 BFF (issue #16): session cookie + token attachment for `/bff/*` — shipped: `crates/server/src/bff/` (cloud#16)

## 6. Wrap-up

- [x] 6.1 OpenAPI regenerated (`cargo xtask openapi`) and client types regenerated (`pnpm api:generate`; no change to `schema.gen.ts`)
- [x] 6.2 CHANGELOG `[Unreleased]`, `docs/architecture.md` and `AGENTS.md` updated
- [x] 6.3 Gates green: fmt, clippy, tests, `cargo xtask arch`, `cargo xtask openapi --check`, sqlx prepare check, `cargo deny`, `cargo machete`, client lint/typecheck/test/api:check, `openspec validate --all --strict`
- [x] 6.4 UI: none (no client change)
