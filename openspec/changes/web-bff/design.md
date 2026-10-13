# Design

## Context

ADR 0004: the API is an OAuth 2.1 resource server, the IdP is Rauthy in its
own container, and the web app talks to the API through a backend-for-frontend
in `campbooks-server` so browser JavaScript never sees a token. The Rauthy
spike (ADR 0004, Evaluation) recorded what this design relies on:

- Issuer `${CAMPBOOKS_IDP_URL}/auth/v1/` (trailing slash). In compose the
  server reaches it at `http://idp:8080/auth/v1/`; browsers use the public URL.
- Client `campbooks-web`: confidential, authorization code + PKCE (S256 only),
  `refresh_token` flow, redirect `${CAMPBOOKS_PUBLIC_URL}/bff/callback`,
  post-logout `${CAMPBOOKS_PUBLIC_URL}/`, access tokens 15 min, EdDSA.
- A refresh token is valid only from `access_token_lifetime − 60 s`; using it
  earlier fails **and makes Rauthy revoke the user's linked sessions**
  (`disable_refresh_token_nbf`, Rauthy's config reference). Refresh tokens
  rotate (5 s grace).
- Discovery (verified on 0.36.2): `revocation_endpoint`, `end_session_endpoint`,
  `backchannel_logout_supported` and `backchannel_logout_session_supported`
  are all present. ID tokens carry `sid`.

### Data ownership

Schema **`bff`**, owned by `campbooks-server`'s BFF, not by a module (like
`kernel`): migrations `<ts>_bff__<desc>.sql`, touching only `bff`, checked by
`cargo xtask arch`; no module may touch it or take its name. No cross-schema
reference. The runtime role `campbooks_app` (row-level security, #442) is
granted the two tables; they carry no workspace policies, because a session
belongs to a person at the IdP, not to a workspace. No domain events are
published or consumed.

Operation catalogue (ADR 0013, #444): every `/bff/*` operation has an
`operationId` `bff.<verb>` and an `x-campbooks` block (`mutates` only for
POST/PUT/PATCH/DELETE, never offered to automation or the assistant).
`cargo xtask arch` checks them like a module's, owned by `bff`. They are
not in the invoke catalogue: the server composes that from the modules'
routes only. `/bff/api/*` dispatches into those composed routes, so
`Idempotency-Key` works through the BFF exactly as on `/api/*`. The outer
route's `MatchedPath` is dropped first, so the layer still recognises the
operation. Background work:
`bff.prune_sessions` (queue `maintenance`, every 15 min), registered through
`Registry::build_with` because the BFF is not a module.

### OAuth scopes per endpoint

The BFF's own endpoints take no bearer token, so they need no scope. They are
documented with explicit security: `security(())` for `/bff/login`,
`/bff/callback` and `/bff/backchannel-logout` (the last is authenticated by
the signed logout token); `security((), ("session" = []))` for `/bff/session`;
`security(("session" = []))` for `/bff/logout` and `/bff/api/{path}` (one
operation per method). `session` is a new OpenAPI `apiKey`-in-cookie scheme;
`cargo xtask arch` allows it only on `/bff/*` paths and only without scopes.
What `/bff/api/*` may do is whatever the session's access token's scopes
allow: the BFF requests `openid profile email offline_access` plus the whole
scope catalogue (override: `CAMPBOOKS_BFF__SCOPES`), and the API checks the
scope of each endpoint exactly as for any other client.

## Decisions

### Endpoints and flow

1. `GET /bff/login?return_to=` validates `return_to` (a path on this site:
   `/`-rooted, not `//`, no backslash, quotes, angle brackets, whitespace or
   control characters, at most 2 KiB, not under `/bff`), makes four 256-bit
   random values (state, nonce, PKCE verifier, browser token), stores the
   sign-in in `bff.login_transactions` (state and browser token as SHA-256,
   verifier encrypted, 10 min), sets the login cookie to the browser token and
   answers 303 to the IdP's authorization endpoint.
2. `GET /bff/callback` deletes the sign-in named by `state` (single use,
   whatever happens next), requires the login cookie to hash to its browser
   hash (login CSRF), checks expiry, an absent `error`, and an `iss` (RFC 9207)
   equal to the issuer when present; exchanges the code with the verifier;
   validates the ID token; deletes the browser's previous session, if any;
   creates the session and answers **200 with a same-origin landing page**:
   `<meta http-equiv="refresh">` to `return_to` and a plain link, under a CSP
   allowing nothing (`default-src 'none'`), `Referrer-Policy: no-referrer`,
   `Cache-Control: no-store`. The login cookie is cleared on every answer.
3. `GET /bff/session` → `{authenticated, user: {sub, name, email},
   expires_at}`; `{authenticated: false}` without a live session (and the
   stale cookie is cleared). Never a token.
4. `/bff/api/*` (GET, POST, PUT, PATCH, DELETE): the session's access token
   (refreshed if due) as `Authorization: Bearer`, the browser's `Cookie`,
   `Authorization` and CSRF headers removed, the path's `/bff` prefix removed,
   then `oneshot` through the API router built for this purpose (module routes
   only: no middleware, no web client, no `/openapi.json`). The API validates
   the token like any other.
5. `POST /bff/logout`: delete the session, revoke its refresh token
   (RFC 7009, `token_type_hint=refresh_token`), clear the cookie, return
   `{logout_url}`: the IdP's end-session endpoint with `client_id` and
   `post_logout_redirect_uri` (OIDC RP-Initiated Logout 1.0 §2). It sends no
   `id_token_hint`, because that would put the ID token in browser
   JavaScript. Rauthy 0.36 then asks the user to confirm and does not return
   to the app; see `decisions-needed.md`.
6. `POST /bff/backchannel-logout` (form `logout_token`): validated by the
   kernel (signature, issuer, `aud` = client id, `iat`, `exp`, the
   back-channel event, no `nonce`, `sub` and/or `sid`); deletes the sessions
   of that IdP session (`sid`, and `sub` when both are given) or, with `sub`
   only, all of the user's. 200 on success, 400 `invalid_logout_token`
   otherwise. A replayed token can only end sessions that the same IdP session
   already ended, so `jti`s are not tracked.

### Why the callback answers with a page, not a redirect

The IdP sends the browser to `/bff/callback` with a cross-site redirect.
Browsers treat a redirect chain that started cross-site as cross-site to the
end, so a `302` from the callback to `return_to` would arrive without the
`SameSite=Strict` session cookie it just set, and the first page load would
look signed out. A 200 page that navigates itself (`meta refresh`) starts a
new, same-site navigation that carries it. The login cookie has the opposite
problem: it must arrive *on* that cross-site redirect, so it is
`SameSite=Lax` (sent on top-level `GET` navigations), short-lived and
single-use. Verified in Chromium against Rauthy (task 7.2).

### Session store

`bff.sessions`: `id` (UUIDv7), `token_hash` (SHA-256 of the cookie token,
unique), `previous_token_hash` + `rotated_at` (rotation grace), `subject`,
`idp_session_id` (`sid`), `name`, `email`, `access_token`,
`access_expires_at`, `refresh_token`, `id_token` (all three encrypted),
`user_agent` (first 256 characters), `created_at`, `last_seen_at`,
`idle_expires_at`, `expires_at`.

- **Cookie value**: 256 random bits (base64url). The database holds only its
  SHA-256, so a read of the table cannot be replayed as cookies.
- **Lookup**: by hash, current token or (for `ROTATION_GRACE` = 2 min after a
  rotation) the previous one, and only while `idle_expires_at` and
  `expires_at` are in the future. Use is recorded at most once a minute.
- **Timeouts**: idle `CAMPBOOKS_BFF__IDLE_TIMEOUT_MINUTES` (48 h, Rauthy's
  refresh-token lifetime: an idle session could not refresh anyway), absolute
  `CAMPBOOKS_BFF__ABSOLUTE_TIMEOUT_HOURS` (30 days). The cookie's `Max-Age`
  is the absolute remainder.
- **Rotation**: a new session (and id) on every sign-in, the browser's old one
  deleted (no fixation); then every `CAMPBOOKS_BFF__ROTATE_MINUTES` (60) the
  first request with the current token swaps it (`UPDATE … WHERE token_hash =
  old`, so concurrent requests rotate once) and sets the new cookie; requests
  still in flight with the old token keep working for two minutes.
- **Pruning**: `bff.prune_sessions` deletes sessions past either deadline and
  sign-ins past theirs.

### Encryption at rest

AES-256-GCM (`aws-lc-rs`, already the crypto backend of rustls and
jsonwebtoken), key = HKDF-SHA256(`CAMPBOOKS_SECRET_KEY`, info
`"campbooks bff token encryption v1"`), so the instance secret can key other
features later without reuse. Each value is `0x01 ‖ 96-bit random nonce ‖
ciphertext ‖ tag`, with associated data `bff.sessions:<id>:<column>` (or the
state hash for a PKCE verifier) so a ciphertext cannot be moved between rows or
columns. Random nonces are safe far beyond this volume (2^32 encryptions per
key).

`CAMPBOOKS_SECRET_KEY` is base64 of 32 random bytes, from the environment or
`CAMPBOOKS_SECRET_KEY_FILE`; the server refuses to start with web sign-in
configured and no key. In compose, `server-init` generates it once into the
`server-secrets` volume. **Rotation**: replace the key and restart; sessions
whose tokens no longer decrypt end on their next use (the user signs in
again) and are pruned. Nothing else is encrypted with it yet, so that is the
whole cost; keeping a previous key for a transition is a follow-up for when
something durable depends on it.

### Refresh

The access token's lifetime comes from the token response's `expires_in`,
counted from when the server received it, so its deadline is never earlier
than the IdP's. A request refreshes when less than 30 s remain
(`REFRESH_BEFORE_EXPIRY`): later than Rauthy's `exp − 60 s` floor, so a
refresh never comes early (which Rauthy punishes by revoking the user's
sessions), and early enough not to send expired tokens. Single-flight: an
in-process lock (64 stripes by session id), then `SELECT … FOR UPDATE` on the
row in a transaction held across the IdP call, re-checking the deadline after
acquiring it, so concurrent requests in any number of `api` processes refresh
once and the rest use the result. The rotated refresh token is stored (the
old one kept if the IdP returns none). `invalid_grant` (or any 4xx) ends the
session → 401 `session_required`. An unreachable IdP keeps the current access
token while it is still valid, otherwise 503.

### Client secret and IdP addresses

`client_secret_post` for every IdP call (Rauthy rejects Basic for
client_credentials; post works for all grants). The secret is
`CAMPBOOKS_BFF__CLIENT_SECRET_FILE` (compose) or `…__CLIENT_SECRET`
(development), read on first use and re-read after `invalid_client`, so the
server may start before `idp-provision` has written it and a rotated secret
needs no restart. Discovery is fetched from `CAMPBOOKS_BFF__IDP_INTERNAL_URL`
(or the issuer) and must name the configured issuer; token and revocation
endpoints under the public issuer are rewritten to the internal one, while
the authorization and end-session endpoints stay public.

### CSRF

The session cookie is `SameSite=Strict`, which already keeps it off
cross-site requests. Independently, every unsafe request to `/bff/logout` and
`/bff/api/*` needs `X-Campbooks-CSRF: 1` (a custom header needs a CORS
preflight the server never approves) **and** a same-origin signal:
`Sec-Fetch-Site: same-origin`, or, only when the browser sends no fetch
metadata, `Origin` equal to `CAMPBOOKS_PUBLIC_URL`'s origin. Failures are
403 `csrf_check_failed`. The back-channel endpoint is exempt (server to
server, authenticated by its token).

### Compose

`server-init` (busybox, one-shot) owns the `server-secrets` volume: generates
`secret-key` once, makes the directory `10001:10001 0750`. `idp-provision`
(uid 10001) writes `campbooks-web.client-secret` there `0440`. The server
mounts the volume read-only and reads both through `group_add: ["10001"]`.
The secret does not stay only in `idp-data`, which the server could not read
(Rauthy's files are `0600`) and which also holds the IdP's admin API key.
`CAMPBOOKS_BFF__IDP_INTERNAL_URL` is `http://idp:8080/auth/v1/`. The
back-channel URL defaults to `${CAMPBOOKS_PUBLIC_URL}/bff/backchannel-logout`
when the public URL is https: Rauthy's HTTP client refuses plain http unless
it runs in dev mode, so an in-network `http://server:8080` target cannot work.

## Threat model

| Threat | Mitigation |
| --- | --- |
| XSS reads tokens | No token ever reaches the browser: not in cookies (opaque id), not in `/bff/session`, not in the logout URL. Session cookie is `HttpOnly`. |
| XSS acts as the user | Out of scope for a BFF (same-origin script can call `/bff/api`); the app's CSP and safe-HTML rules remain the defence. |
| Stolen database dump | Cookie tokens stored as SHA-256; access, refresh and ID tokens encrypted with a key that is not in the database. |
| Ciphertext moved between rows | AEAD associated data binds each value to its row and column. |
| CSRF on API writes or logout | `SameSite=Strict`, plus custom header and `Sec-Fetch-Site`/`Origin` check on every unsafe method. |
| Login CSRF (victim signed in as the attacker) | The callback requires the login cookie set when *this* browser started the sign-in. |
| Authorization-code injection / replay | PKCE S256 verifier kept server-side; state single-use and expiring; code single-use at the IdP. |
| ID-token substitution / replay | Signature via the IdP's JWKS, issuer, `aud` = client id, `azp`, nonce bound to the sign-in, `exp`/`iat`; header `typ` must not be `at+jwt`. |
| IdP mix-up | One issuer, discovery must name it, RFC 9207 `iss` checked when present. |
| Open redirect via `return_to` | Only same-site paths (no scheme, no `//`, no backslash), never under `/bff`; rejected with 400 otherwise. |
| Session fixation | A new session id at every sign-in, the old session deleted. |
| Stolen cookie | `Secure`, `__Host-`, idle and absolute timeouts, hourly rotation, logout and IdP back-channel logout end it. |
| Cookie tossing from a sibling subdomain | `__Host-` prefix; a duplicated cookie name is treated as absent. |
| Token leakage in logs | Request logs record the path only (the callback URL carries the code); IdP errors log the `error` code only; tokens, codes, cookies and session ids are never logged. |
| Early refresh revoking all sessions (Rauthy) | Refresh only in the last 30 s before expiry, computed from `expires_in`; single-flight. |
| Forged back-channel logout | Signed logout token from the configured issuer for this client; worst case of a replay is ending sessions already ended at the IdP. |
| Sign-in table flooding | Abandoned sign-ins expire after 10 min and are pruned every 15 min; rate limiting is a follow-up. |

## Risks / Trade-offs

- **The refresh transaction holds a row lock across an IdP call** (≤ 10 s
  timeout). Only requests of the same session wait; it buys cross-process
  single-flight without another lock service.
- **`/bff/api/*` requires a session even for public API routes.** Simple and
  predictable; the web app is a signed-in app. The home page's system status
  therefore needs a sign-in now.
- **Logout UX with Rauthy 0.36**: without `id_token_hint` Rauthy shows a
  confirmation and stays on its own page. Recorded in `decisions-needed.md`.
- **Back-channel logout needs https** between the IdP and the server's public
  URL. On plain-http local setups it is off; sessions end by timeout or
  logout.
