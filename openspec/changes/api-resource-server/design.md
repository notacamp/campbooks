# Design

## Context

`kernel::auth` holds a placeholder `Scope` and `Principal`. Handlers receive
`AppState` (db, config, event bus). Errors are `kernel::Error`, rendered as
RFC 9457 problem+json. Modules hand the server a `Module { name, router }`,
and the server merges the routers into one OpenAPI document that `cargo xtask
openapi --check` keeps committed. See proposal.md for why this is needed now.

The IdP spike (ADR 0004 in #431) recorded these Rauthy token facts:

- The issuer is `${CAMPBOOKS_IDP_URL}/auth/v1/`, with the trailing slash.
  Discovery is at `/auth/v1/.well-known/openid-configuration`. JWKS is at
  `/auth/v1/oidc/certs`, reached in compose as `http://idp:8080/...`.
- Tokens are signed EdDSA (Ed25519) or RS256, per client. Header `typ` is
  `JWT` on 0.36 and `at+jwt` from 0.37.
- `aud` is a string or an array and contains `campbooks-api` for our clients.
- `scope` is space-separated and `openid` is always present.
- `client_credentials` tokens carry no `sub`. `azp` is the client id.
- Per-workspace M2M clients carry `workspace_id`, at the root
  (`claims_at_root`) or under `custom`.

Data ownership: none. There is no schema, no migration, and no event
published or consumed.

OAuth scopes per endpoint: `GET /api/v1/system/health` and
`GET /api/v1/system/version` are **public** (`security(())`). No endpoint
requires a scope yet. `system:read` is declared for the catalogue only.

## Goals / Non-Goals

**Goals:**

- One extractor, `Scoped<S>`, is the only way a handler gets a caller. Its
  failure modes are uniform and standard (RFC 6750 + RFC 9457).
- Fail closed. A missing or broken IdP config never lets a request through.
- A scope is written once, and the IdP, OpenAPI and `cargo xtask arch` all
  derive from that declaration.
- Everything is testable offline, with real signatures and real HTTP over
  loopback.

**Non-Goals:** see proposal.md (BFF, IdP bootstrap, per-record permissions,
introspection, DPoP).

## Decisions

### Configuration

`kernel::config::AuthConfig`, read as `CAMPBOOKS_AUTH__*`:

| Setting | Default | Notes |
| --- | --- | --- |
| `ISSUER` | unset | compared byte for byte with `iss`; blank counts as unset |
| `AUDIENCE` | `campbooks-api` | must appear in `aud` |
| `JWKS_URL` | unset → discovery | set it when the IdP is on an internal address |
| `LEEWAY_SECONDS` | `60` | for `exp`, `nbf`, `iat` |

At startup the server checks that the issuer and JWKS URL are `http(s)` URLs
and that the audience is not blank, and refuses to start otherwise, naming
the setting. With no issuer it starts, logs one warning, and the verifier
rejects everything with `auth_not_configured`. Keys are fetched lazily, not
at startup, so an IdP that boots after the API does not fail the API.

### Validation

`jsonwebtoken` 11.1.0 with the `aws_lc_rs` backend (MIT; aws-lc-rs is already
the rustls crypto provider; no PEM, no OpenSSL). It was released 2026-09-16,
more than two weeks before this change. Validation runs in this order, and
any failure is 401 `invalid_token` unless noted:

1. Decode the header. `alg` must be in {EdDSA, RS256}, so `none` and HMAC are
   rejected before any key lookup. `typ`, when present, must be `JWT` or
   `at+jwt` (case-insensitive, `application/` prefix allowed). `kid` is
   required.
2. Look up the key by `kid` **and** `alg`. A JWKS entry is usable only if it
   is a signing key (`use` absent or `sig`), its type fits the algorithm (OKP
   Ed25519 → EdDSA, RSA → RS256), and its own `alg`, when given, equals that
   algorithm. An RS256 header naming an Ed25519 key therefore finds no key.
3. Verify the signature. `iss` must equal the issuer. `aud` must contain the
   audience. `exp`, `iss` and `aud` are required. `nbf` is checked with
   leeway. An expired token is 401 `token_expired`.
4. `iat`, when present, may not be more than the leeway in the future.
5. Build the `Principal`. `subject` is `sub`, or `azp` when `sub` is absent.
   A token with neither is invalid. `kind` is `Client` when there is no `sub`
   or `sub == azp`, and `User` otherwise. `client_id` is `azp`. `scopes` come
   from splitting `scope` on whitespace. `workspace` is `workspace_id` from the
   root or `custom.workspace_id`. If both are present and differ, the token is
   invalid.

`workspace` is a `String`. The kernel defines no workspace id type, and the
IdP sets the value as an opaque client claim. A module that owns workspaces
parses it.

### JWKS cache

One cache per verifier, shared by every clone, in `AppState::auth`:

- **Source:** `JWKS_URL`, or else the discovery document's `jwks_uri`. The
  discovery document's `issuer` must equal the configured one (OIDC Discovery
  §4.3). The discovered URL is remembered and forgotten after a failed fetch.
- **TTL:** the response's `Cache-Control: max-age`, defaulting to 1 h and
  capped at 24 h.
- **Refresh triggers:** an empty cache, an expired cache, or a token whose
  `kid`/`alg` is not cached.
- **Single-flight:** one refresh runs at a time behind an async mutex. A
  request that waited while another fetch started uses that fetch's result
  instead of fetching again.
- **Rate limit:** while keys are cached, at most one fetch starts per 30 s.
  Tokens with made-up `kid`s therefore cost at most one IdP request per 30 s.
  With an empty cache every request may trigger a fetch, still single-flight,
  so the first request after the IdP comes back succeeds.
- **Failure:** a failed refresh keeps the last good keys and logs a warning
  with the URL and error, never a token. If no keys were ever fetched, the
  request fails with **503**: the token may be fine, and a 401 would push
  clients into a pointless re-login.
- Responses over 1 MiB, invalid JSON, and sets with no usable key count as
  failures. Keys of unknown types are skipped one by one, so an encryption key
  or a new curve in the set does not break validation.

The cache times itself with the kernel `Clock`, so tests move a `FixedClock`
through TTLs and cooldowns. Token `exp`/`nbf`/`iat` always use the system
clock, as `jsonwebtoken` does.

HTTP goes through the workspace's existing `reqwest` with rustls and a 10 s
timeout. The client is built lazily, so `AppState::new` stays infallible.

### Scope catalogue

```rust
pub struct MailRead;
impl ScopeName for MailRead {
    const NAME: &'static str = "mail:read";
    const DESCRIPTION: &'static str = "Read your mail.";
}
pub const ALL: &[ScopeInfo] = &[ScopeInfo::of::<MailRead>()];
// Module { name: "mail", scopes: ALL, router: … }
```

The type is what the handler names (`Scoped<MailRead>`). The const list is
what the module registers. Both come from the same `impl`, so a scope's
name and description live in one place. `campbooks_server::scope_catalogue()`
merges and sorts them. `campbooks-server scopes` prints
`[{name, description}]` for the IdP bootstrap, and the OpenAPI `bearer`
scheme's description lists them.

### Extractor and errors

`Scoped<S: ScopeName>` implements `FromRequestParts<St>` for any state from
which a `TokenVerifier` can be taken (`FromRef`). `AppState` provides one, and
tests can use a bare verifier as state. It reads exactly one `Authorization`
header and yields `Scoped { principal, .. }`.

| Situation | Status | `detail` | `WWW-Authenticate` |
| --- | --- | --- | --- |
| no issuer configured | 401 | `auth_not_configured` | `Bearer` |
| no header, or a non-Bearer scheme | 401 | `missing_token` | `Bearer` |
| empty/duplicate/malformed/forged/foreign token | 401 | `invalid_token` | `Bearer error="invalid_token"` |
| expired token | 401 | `token_expired` | `Bearer error="invalid_token"` |
| keys never fetchable | 503 | message | none |
| valid token, scope missing | 403 | `insufficient_scope` | `Bearer error="insufficient_scope", scope="<S>"` |

A request with no credentials gets a bare `Bearer` challenge, following RFC
6750 §3.1 ("SHOULD NOT include an error code"), rather than
`invalid_token`. `detail` is a short code and never includes claim values,
key ids or the token. Tokens are never logged. A rejected token logs only its
reason, at `debug`.

`Error::Unauthorized(AuthFailure)` and `Error::InsufficientScope(&'static str)`
are about the token. Whether a principal may see a given record is the
module's decision, made in the handler or query. A read the caller may not
see is `Error::NotFound` (404), never 403. `Error::Forbidden` stays for
refused *actions*.

### OpenAPI and `cargo xtask arch`

The server adds the `bearer` scheme (HTTP bearer, `bearerFormat: JWT`) to
`components.securitySchemes`. Scoped handlers declare
`security(("bearer" = ["<scope>"]))`, and public ones declare `security(())`,
which renders as `[{}]`. No document-wide default is set, so an operation
with no declaration is caught. `cargo xtask arch` reads the generated document
and fails on:

- an operation without `security`;
- a requirement naming a scheme other than `bearer`;
- a `bearer` requirement with no scope, or with a scope no module declares;
- a scope name that is not `<module>:<action>` (lower-case) of the declaring
  module, a duplicate, or one with no description.

The arch check cannot see whether a handler really takes `Scoped<S>`. A
server test covers that pairing instead: it calls every operation the
document marks as scoped without a token and expects 401.

### Test identity provider

`campbooks_kernel::testing::TestIdp` (feature `test-util`, and the kernel's
own tests) serves discovery and JWKS on `127.0.0.1:0` with Rauthy's paths,
signs Ed25519 tokens with `typ: at+jwt`, rotates keys, can mint tokens from
unpublished keys, sets `max-age`, simulates an outage, and counts fetches.
`idp.state(db)` returns an `AppState` that trusts it. The kernel's end-to-end
tests build their protected route inside the test. No demo endpoint is added
to the API.

## Risks / Trade-offs

- [`jsonwebtoken` pulls `aws-lc-rs` default features, adding `untrusted`
  0.7 next to 0.9] → only a duplicate-version warning (ISC, already allowed).
  Our own use of `aws-lc-rs` is test-only.
- [RS256 is accepted but only exercised by unit tests of key selection, not
  by a signed end-to-end token] → `aws-lc-rs` cannot export the PKCS#1 key
  `jsonwebtoken` signs RSA with. Verification is `jsonwebtoken`'s own, and
  the Rauthy default is EdDSA.
- [IdP down at first request] → 503 until it answers. Once keys are cached,
  an outage is invisible until the cached keys expire, and even then the last
  good keys keep working.
- [Header `kid` required] → Rauthy always sets it. A key-less token would
  otherwise force trying every key.
- [`iat` must be an integer] → Rauthy issues integers. A fractional `iat` is
  rejected as invalid rather than silently accepted.

## Migration Plan

None for operators. No endpoint requires a scope yet, so instances without
`CAMPBOOKS_AUTH__*` behave as before, apart from the startup warning. Once
#431 lands, compose sets `CAMPBOOKS_AUTH__ISSUER` and
`CAMPBOOKS_AUTH__JWKS_URL`. Rollback is redeploying the previous image.
