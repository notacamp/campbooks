# Proposal

## Why

ADR 0004 makes the API a pure OAuth 2.1 resource server: every request to a
protected endpoint carries a JWT access token from the instance's identity
provider, and every handler checks a scope. Today `kernel::auth` is a
placeholder. No endpoint can be protected, the scopes the IdP should know
about are not written down anywhere, and nothing stops a new endpoint from
shipping without any access rule. That has to be in place before the first
domain module (mail, calendar, …) adds endpoints that return user data.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/15. The
IdP is Rauthy (spike in notacamp/campbooks#431, not merged yet). This change
is designed against the token shape that spike recorded and needs no running
IdP: tests sign their own tokens.

## What Changes

- New `CAMPBOOKS_AUTH__*` settings: issuer, audience (default
  `campbooks-api`), JWKS URL (discovered from the issuer when unset) and clock
  leeway (default 60 s). With no issuer the server still starts, logs one
  warning, and every scoped endpoint answers 401 `auth_not_configured`. It
  never lets requests through.
- `kernel::auth` validates access tokens: the signature against a key from
  the IdP's JWKS chosen by `kid`, `alg` limited to EdDSA and RS256 and
  matching the key, exact issuer, our audience in `aud`, required `exp`,
  `nbf`/`iat` checked with leeway, and header `typ` either `JWT` or `at+jwt`.
  The JWKS is fetched lazily, cached by `Cache-Control: max-age`, refreshed
  when a token names an unknown `kid` (at most once per 30 s, single-flight),
  and a failed refresh keeps the last good keys.
- `Principal` gains `kind` (user or client), `client_id` (`azp`) and
  `workspace` (`workspace_id` at the token root or under `custom`).
  `client_credentials` tokens, which have no `sub`, use `azp` as subject.
- Scopes are declared once, as types implementing `ScopeName`, listed in each
  module's `Module` descriptor. The server builds the catalogue from them, and
  `campbooks-server scopes` prints it as JSON so the IdP bootstrap can mirror
  it. The `system` module declares `system:read`.
- The `Scoped<S>` extractor gives handlers the `Principal`. Token problems
  return 401 problem+json with an RFC 6750 `WWW-Authenticate` challenge, and a
  valid token without the scope returns 403 `insufficient_scope` naming the
  scope.
- OpenAPI gains the `bearer` security scheme (HTTP bearer, JWT), whose
  description lists the catalogue. Every operation states its security
  explicitly. Health and version are explicitly public.
- `cargo xtask arch` fails when an OpenAPI operation declares no `security`,
  names an unknown scheme or a scope no module declares, or requires a token
  without a scope, and when a scope name is not `<module>:<action>` of the
  module that declares it.
- `campbooks_kernel::testing::TestIdp`: an in-process IdP (discovery + JWKS
  on loopback, Ed25519 signing) so module tests can exercise scoped endpoints
  end to end.

## Capabilities

### New Capabilities

- `api-auth`: how the API authenticates callers (bearer JWTs from the
  configured IdP), authorizes them per endpoint (scopes), declares scopes, and
  documents and enforces all of this.

### Modified Capabilities

None. `system` health and version stay public; they now say so explicitly in
the OpenAPI document.

## Impact

- Modules: `campbooks-kernel` (auth, config, error, module contract, state,
  test helpers), `campbooks-server` (catalogue, OpenAPI scheme, `scopes`
  subcommand, startup check), `campbooks-system` (declares `system:read`;
  marks its endpoints public), `xtask` (two new arch rules). No new module, no
  cross-module dependency, no `architecture.toml` entry.
- `kernel::Module` gains a `scopes` field. `kernel::Error::Unauthorized` now
  carries an `AuthFailure`, and a new `InsufficientScope` variant was added.
  Nothing outside the kernel used `Unauthorized` yet.
- Dependencies: `jsonwebtoken` 11.1.0 (MIT) with its `aws_lc_rs` backend,
  which is the crypto library rustls already uses. No PEM support and no
  OpenSSL. The kernel now also uses the workspace's existing `reqwest`
  (rustls) to fetch the JWKS. Tests use `aws-lc-rs` and `base64`, which are
  already in the graph.
- Self-hosters: new optional `CAMPBOOKS_AUTH__*` settings. Nothing breaks
  without them: no endpoint requires a scope yet.
- OpenAPI: one security scheme and explicit `security` on every operation.
  Generated client types do not change.

## Non-goals

- The BFF (session cookie, OIDC code flow, token attachment for `/bff/*`):
  issue #16. The interim `/bff/api/*` passthrough stays credential-free, so
  scoped endpoints answer it 401 like a direct call.
- Mirroring the catalogue into the IdP. `deploy/idp/*` lives in the unmerged
  #431. That bootstrap should read `campbooks-server scopes`.
- Workspace membership, roles, or per-record permissions. These are module
  concerns: a read the caller may not see answers 404.
- Token introspection, revocation lists, DPoP, or opaque tokens. Access
  tokens are short-lived self-contained JWTs (ADR 0004).
- Any scoped endpoint in the real API. None exists yet, and none is added
  here only for demonstration.
