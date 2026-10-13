# Proposal

## Why

ADR 0004 says web browsers never hold a token: the server runs the OpenID
Connect flow itself and gives the browser an httpOnly, `SameSite=Strict`
session cookie. Until now `/bff/api/*` was an interim passthrough with no
session and no credentials, so the web app could only call public endpoints,
and nobody could sign in to it. The IdP (Rauthy, #431), the resource server
(#433) and the job queue (#434) are in place; this change adds the piece
between them.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/16.

## What Changes

- **Sign-in.** `GET /bff/login?return_to=/path` starts the authorization-code
  flow with PKCE (S256), `state` and `nonce`, bound to the browser by a
  short-lived, single-use `SameSite=Lax` login cookie. `GET /bff/callback`
  checks the state, exchanges the code (`client_secret_post`), validates the
  ID token with the kernel's JWKS machinery (issuer, audience, `azp`, nonce,
  expiry, signature), starts a session and lands on `return_to` through a
  small same-origin page (a redirect at the end of the IdP's cross-site
  redirect chain would not carry the new `Strict` cookie).
- **Sessions** in Postgres, schema `bff`, owned by the server's BFF: an
  opaque 256-bit cookie token of which only the SHA-256 is stored; access,
  refresh and ID tokens encrypted with AES-256-GCM under a key derived from
  the new instance secret `CAMPBOOKS_SECRET_KEY`. Idle (48 h) and absolute
  (30 days) timeouts, hourly id rotation with a two-minute grace, a new id on
  every sign-in. Ended sessions are pruned every 15 minutes by the scheduled
  job `bff.prune_sessions`.
- **Cookie** `__Host-campbooks_session` (`HttpOnly`, `Secure`,
  `SameSite=Strict`, `Path=/`, no `Domain`). When `CAMPBOOKS_PUBLIC_URL` is
  plain http (local development) it is `campbooks_session` without `Secure`,
  and the server says so once at startup.
- **`GET /bff/session`** says who is signed in (`sub`, name, email, when the
  session ends) and never returns a token.
- **`/bff/api/*`** replaces the passthrough: it needs a live session,
  refreshes the access token just before it expires (single-flight per
  session, across processes), and runs the request through the API router
  in-process with `Authorization: Bearer` attached. Without a session it
  answers 401 `session_required`.
- **CSRF**: unsafe methods on cookie-authenticated endpoints need
  `X-Campbooks-CSRF: 1` and `Sec-Fetch-Site: same-origin` (or, without fetch
  metadata, an `Origin` equal to the public URL's); otherwise 403
  `csrf_check_failed`.
- **`POST /bff/logout`** ends the session, revokes its refresh token at the
  IdP, clears the cookie and returns the IdP's end-session URL.
  **`POST /bff/backchannel-logout`** accepts the IdP's signed logout tokens
  (OIDC Back-Channel Logout 1.0) and ends the sessions they name.
- **Kernel**: `TokenVerifier::verify_id_token` and `verify_logout_token`
  share the access-token verifier's key cache; `Registry::build_with` lets
  the server register background work owned by something other than a module;
  `testing::TestIdp` gains a token endpoint (authorization code with PKCE,
  rotating refresh tokens), revocation, `authorize`, ID tokens and logout
  tokens.
- **Architecture rules**: `bff` joins `kernel` as an infrastructure schema
  (`<ts>_bff__<desc>.sql`, touching only `bff`); OpenAPI gains a `session`
  cookie security scheme that only `/bff/*` operations may use.
- **Client**: the web transport sends the CSRF header and, on
  `session_required`, sends the browser to `/bff/login?return_to=<current
  page>` (once). `useSession()` reads `/bff/session`. The Vite dev server
  proxies `/bff` to the server unchanged instead of faking the passthrough.
- **Compose**: a `server-init` one-shot generates `CAMPBOOKS_SECRET_KEY` into a
  new `server-secrets` volume; `idp-provision` writes the web client's secret
  there (not into `idp-data`, which also holds the IdP's admin API key) and
  registers the back-channel logout URL when the public URL is https. The
  server mounts the volume read-only.
- HTTP request logs record the path, no longer the query string (it carried
  the authorization code).

## Capabilities

### New Capabilities

- `web-bff`: how a browser signs in to Campbooks and calls the API without
  ever holding a token.

### Modified Capabilities

None. The interim passthrough of `serve-web-client` was a design detail of
that change, not a specified requirement.

## Impact

- Code: `campbooks-server` (new `bff` module, router, startup), `campbooks-kernel`
  (OIDC token validation, `Registry::build_with`, test IdP), `xtask` (arch
  rules), `clients/app/src/lib/api`.
- Data: new schema `bff` (two tables, granted to the `campbooks_app` runtime role
  without row-level security: sessions belong to people, not workspaces),
  migration `20261013170000_bff__sessions.sql`.
  No module schema changes; no events published or consumed.
- Configuration: `CAMPBOOKS_SECRET_KEY` / `CAMPBOOKS_SECRET_KEY_FILE` (required
  once web sign-in is configured) and `CAMPBOOKS_BFF__*`. Without an issuer or
  a client secret, web sign-in is off and `/bff/*` answers 503 (except
  `/bff/session`).
- Dependencies: none new. `aws-lc-rs`, `base64`, `reqwest`, `url` and `uuid`
  were already in the workspace; axum gains its `form` feature.
- Behaviour: `/bff/api/*` now requires a session; the web app's home page sends
  a signed-out visitor to sign in.

## Non-goals

- Sign-in, sign-out or account screens in the web app (the design system lands
  separately). Today the IdP's own pages are the only UI.
- Serving the IdP under the app's origin (`/auth/*`); it keeps its own origin
  (see `openspec/decisions-needed.md`).
- Native (Tauri) sign-in: campbooks-cloud#17.
- Workspaces, membership and roles: unchanged, keyed by the IdP subject later.
- A "your devices" page listing sessions (the stored user-agent hint is for it).
- Rate limiting `/bff/login` (pruning bounds abandoned sign-ins; see design).
- Key rotation without signing users out (see design).
