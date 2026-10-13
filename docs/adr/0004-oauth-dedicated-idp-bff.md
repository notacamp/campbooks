# 0004 — OAuth 2.1 with a dedicated IdP; BFF for web, keychain for native

**Status:** Accepted · 2026-10-12 — IdP: **Rauthy** (spike done 2026-10-12, see Evaluation)

## Context
First-party clients (web, desktop, mobile) and third-party clients
(integrations, scripts, MCP agents) must authenticate with the same standard.
Tokens in browser JavaScript are the weakest link.

## Decision
- **A dedicated OIDC/OAuth 2.1 identity provider runs as its own container**
  beside the API: **Rauthy** (Rust, Apache-2.0, passkeys, embedded database
  or Postgres). Ory Hydra with our own login/consent screens was the fallback;
  the spike below confirmed Rauthy (client_credentials, PKCE, refresh
  rotation, API-driven client registration, custom login branding, upgrade
  story for self-hosters).
- **The API is a pure resource server:** validates JWT access tokens
  (issuer, audience, expiry, signature via cached JWKS) and checks a scope on
  every handler. Scopes are a catalogue in the kernel, mirrored in the IdP.
- **Web → BFF.** The server runs the OIDC authorization-code flow itself,
  keeps tokens server-side, and gives the browser only an httpOnly,
  `Secure`, `SameSite=Strict` session cookie. `/bff/*` proxies to the API with
  the token attached. CSRF protection on unsafe methods.
- **Native (Tauri) → public client.** Authorization code + PKCE in the system
  browser (loopback or claimed-scheme redirect). Refresh token in the OS
  keychain/keystore, held by the Rust side; webview JS never sees it.
  Consider DPoP for sender-constrained tokens.
- **External clients:** authorization code + PKCE (user-delegated) and
  client_credentials (machine-to-machine), registered per workspace by the
  API through the IdP's admin API.

## Consequences
- Three more compose services: `idp` (Rauthy) and two one-shot helpers,
  `idp-init` (generates secrets on first boot, writes the config from `.env`)
  and `idp-provision` (registers our scopes and first-party clients once).
  `docker compose up` brings all of it up with no manual step; see
  `deploy/README.md`.
- The IdP keeps its data in Rauthy's embedded database (Hiqlite) in the
  `idp-data` volume, not in our Postgres (see Evaluation → Storage). That
  volume also holds the IdP's encryption key, so it must be backed up either way.
- Login, signup and passkey screens are served by the IdP and themed per
  client (logo, favicon, colours). It is served from its own origin for now
  (`CAMPBOOKS_IDP_URL`, default `http://localhost:8081`).
- Workspace membership and roles stay in Campbooks, keyed by the IdP subject.

## Evaluation (spike, 2026-10-12)

Rauthy 0.36.2 was run in docker compose (`deploy/compose.yaml`, throwaway
project) and exercised with curl and a real browser login; Hydra was assessed
from its documentation only, because Rauthy passed every hard requirement.
"Ran" = verified against the running container; "docs/source" = read in the
upstream docs or source at the pinned tag.

| Must-have | Rauthy 0.36.2 | Evidence | Ory Hydra v26.2 |
| --- | --- | --- | --- |
| Auth code + PKCE (S256 only) | ✅ `plain` and missing challenge rejected per client; code single-use | ran | ✅ |
| client_credentials | ✅ — ⚠️ only with `client_secret_post`: HTTP Basic is rejected for this grant (upstream bug, still on `main`); Basic works for authorization_code | ran | ✅ |
| Refresh-token rotation | ✅ new token on every refresh, old one dead after a 5 s grace — ⚠️ replaying a used token does not revoke its successor | ran | ✅ rotation (reuse handling not documented) |
| JWT access tokens + JWKS | ✅ EdDSA (Ed25519) or RS256/384/512 per client; JWKS at `/auth/v1/oidc/certs`, keys rotate monthly | ran | ✅ (JWT strategy opt-in) |
| Custom scopes | ✅ `system:read` created via API, carried in the `scope` claim | ran | ✅ |
| Audience | ✅ per-client `default_aud` + RFC 8707 `resource` allow-list; unknown resource → `invalid_target` | ran | ✅ `audience` allow-list |
| API-driven client registration | ✅ admin API with a scoped API key (create, configure, read secret); RFC 7591 DCR available but off | ran | ✅ admin API + DCR |
| Per-client custom claims | ✅ `claims` on a client land in its client_credentials tokens (e.g. `workspace_id`) | ran | ⚠️ via token hook |
| Themable login + passkeys | ✅ passkey registration over `http://localhost` and admin login with it; per-client logo/favicon/colours — ⚠️ UI languages: en, de, fr, ko, nb, nl, ru, uk, zh (no pt, es) | ran (passkey), docs (themes) | ❌ headless: we build login/consent, passkeys need Ory Kratos + its UI |
| Social upstream (Google / Microsoft) | ⚠️ Google is a built-in provider type, Microsoft only as generic OIDC; upstream login is labelled beta; not exercised (needs real credentials) | docs/source | ❌ needs Kratos |
| Postgres storage | ✅ supported (`HIQLITE=false` + `PG_*`), one-shot migration between the two (`MIGRATE_DB_FROM`); we use the embedded DB | docs | ✅ (Postgres, MySQL, CockroachDB) |
| arm64 + amd64 image | ✅ multi-arch index, distroless, non-root | ran (`imagetools inspect`) | ✅ |
| Zero-manual-step upgrades | ✅ migrations run on start; 0.36.2 → 0.37.1 on the same volume needed nothing — ⚠️ 0.x minors rename config keys, so every bump is a reviewed PR here, not a self-hoster task | ran | ⚠️ `hydra migrate sql` must run before each start (init container) |
| Maintenance | ✅ Apache-2.0, ~1.4k stars, 63 contributors, releases every 2–6 weeks, security fixes published with advisories — ⚠️ one main maintainer | GitHub | ✅ Apache-2.0, large vendor, slower OSS release cadence (Nov 2025, Mar 2026) |

### Storage: embedded (Hiqlite), not our Postgres
Rauthy supports both. We ship the embedded database because Rauthy needs the
`idp-data` volume regardless (its encryption key, cache WAL and generated
secrets live there). Putting its tables in Postgres would add a database,
a role and an idempotent init step against our Postgres without removing
anything from the backup story. Rauthy also takes a nightly local backup of
the embedded database. Switching later is a supported one-shot migration
(`MIGRATE_DB_FROM`); the hosted cloud may choose Postgres.

### Findings the next issues depend on
- **Issuer** is `${CAMPBOOKS_IDP_URL}/auth/v1/` **with the trailing slash**;
  discovery at `/auth/v1/.well-known/openid-configuration` (also served at the
  RFC 8414 path), JWKS at `/auth/v1/oidc/certs`, token at `/auth/v1/oidc/token`.
- **Access-token claims:** `iss`, `aud` (string for one audience, array for
  several; always contains the client id, plus `campbooks-api` for our
  clients), `azp` (client id), `sub` (user id; absent for client_credentials),
  `scope` (space-separated; `openid` is always added), `exp`/`nbf`/`iat`,
  `jti`, `typ: "Bearer"`, and for users `email`, `email_verified`, `roles`.
  Header `typ` is `JWT` on 0.36 and `at+jwt` from 0.37 (RFC 9068): accept both.
- **Scopes requested at authorize** are honoured for authorization_code; the
  other grants get the client's `default_scopes`, so per-workspace M2M clients
  must be created with the scopes they may use as defaults.
- **Workspace in tokens:** for client_credentials, a client's `claims`
  (`claims_at_root: true`) put e.g. `workspace_id` at the token root. For
  users, workspace stays in Campbooks (keyed by `sub`).
- **Refresh tokens** are issued to clients with the `refresh_token` flow,
  whether or not `offline_access` is requested (we register that scope only
  so clients that send it don't fail). A refresh token is not valid until
  `access_token_lifetime − 60 s` (`nbf`); using it earlier fails with "Token is
  not valid yet". The BFF refreshes on expiry, not eagerly.
- **Native loopback redirects:** clients are registered with
  `http://127.0.0.1/callback` and `campbooks://callback`, and
  `rfc_8252_enable` is on. On 0.37+ that matches any loopback port; on the
  pinned 0.36.2 only the port-less URI and the custom scheme match (0.36
  applies RFC 8252 to dynamic clients only, and 0.37 rejects the `:*`
  wildcard 0.36 needed). Bump to ≥ 0.37.1 before the Tauri sign-in lands.
- **Client secrets:** `idp-provision` stores the `campbooks-web` secret in the
  `idp-data` volume (`campbooks/campbooks-web.client-secret`); the server will
  mount it read-only. M2M clients must authenticate with
  `client_secret_post` until the Basic-auth bug is fixed upstream.

### Settings the server will need (resource server and BFF)
Not added to `crates/kernel/src/config.rs` yet (it would be unused code):
`CAMPBOOKS_AUTH__ISSUER` (e.g. `http://localhost:8081/auth/v1/`),
`CAMPBOOKS_AUTH__AUDIENCE` (`campbooks-api`), `CAMPBOOKS_AUTH__JWKS_URL`
(in compose the in-network `http://idp:8080/auth/v1/oidc/certs`, so the server
needs no route to the public URL), and for the BFF
`CAMPBOOKS_AUTH__WEB_CLIENT_ID` (`campbooks-web`) and
`CAMPBOOKS_AUTH__WEB_CLIENT_SECRET_FILE`
(`/run/idp/campbooks/campbooks-web.client-secret`).
*(The BFF, #16, shipped these as `CAMPBOOKS_BFF__CLIENT_ID` and
`CAMPBOOKS_BFF__CLIENT_SECRET_FILE`, with the secret in a separate
`server-secrets` volume; see `openspec/changes/web-bff/design.md`.)*

## Alternatives considered
- Auth server embedded in the API — one binary, but we would own and audit
  security-critical token issuance ourselves.
- Tokens in the SPA (localStorage/memory) — rejected on criterion (2).
- Ory Hydra (+ Kratos for passkeys and social login) — capable and
  well-funded, but headless: we would build and maintain the login, consent,
  passkey and account screens and run two more services. Kept as the fallback
  if Rauthy's single-maintainer risk materialises.
- Rauthy on our Postgres — supported; not the default, see Storage.
