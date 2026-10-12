## Purpose

How the Campbooks API authenticates and authorizes callers as an OAuth 2.1
resource server (ADR 0004). It accepts JWT access tokens from the instance's
identity provider, requires a scope on every protected endpoint, and states
every endpoint's access rule in the OpenAPI document.

## ADDED Requirements

### Requirement: Identity provider configuration
The server SHALL trust access tokens only from the identity provider named by
`CAMPBOOKS_AUTH__ISSUER`. It SHALL require the audience in
`CAMPBOOKS_AUTH__AUDIENCE` (default `campbooks-api`), fetch signing keys from
`CAMPBOOKS_AUTH__JWKS_URL` or, when that is unset, from the `jwks_uri` of the
issuer's OpenID Connect discovery document, and tolerate
`CAMPBOOKS_AUTH__LEEWAY_SECONDS` (default 60) of clock skew. When no issuer
is configured, the server SHALL still start, log a warning, and refuse every
request to a scoped endpoint. It SHALL never let such a request through. When
a configured issuer or JWKS URL is not an http(s) URL, the server SHALL refuse
to start and name the setting.

#### Scenario: Issuer not configured
- **WHEN** `CAMPBOOKS_AUTH__ISSUER` is unset and a client calls a scoped endpoint, with or without a token
- **THEN** the response is 401 as `application/problem+json` with `detail` = `auth_not_configured`

#### Scenario: Malformed issuer
- **WHEN** the server starts with `CAMPBOOKS_AUTH__ISSUER` set to a value that is not an http(s) URL
- **THEN** it exits with a non-zero status and an error naming `CAMPBOOKS_AUTH__ISSUER`

#### Scenario: JWKS discovered from the issuer
- **WHEN** `CAMPBOOKS_AUTH__JWKS_URL` is unset and the first token is validated
- **THEN** the server reads the issuer's discovery document, which must name the same issuer, and fetches the keys from its `jwks_uri`

### Requirement: Access token validation
A scoped endpoint SHALL accept only a single `Authorization: Bearer` JWT that
meets all of these conditions:
- it is signed by a key from the identity provider's JWKS, selected by the header `kid`;
- its algorithm is EdDSA or RS256 and matches that key's type;
- its `iss` equals the configured issuer exactly;
- its `aud` (a string or an array) contains the configured audience;
- it has an `exp` that has not passed;
- its `nbf` and `iat`, when present, are not in the future beyond the leeway;
- its header `typ`, when present, is `JWT` or `at+jwt`.

Any other token SHALL be refused with 401 as `application/problem+json` and
`WWW-Authenticate: Bearer error="invalid_token"`. A request with no bearer
credentials SHALL be refused with 401 and a bare `WWW-Authenticate: Bearer`
challenge. Response details SHALL be a short reason code and SHALL NOT
include the token or its claims. The server SHALL NOT log tokens.

#### Scenario: Valid token
- **WHEN** a client calls a scoped endpoint with an unexpired token from the configured issuer, for the configured audience, granting the endpoint's scope
- **THEN** the handler runs and the response is that endpoint's success response

#### Scenario: No token
- **WHEN** a client calls a scoped endpoint without an `Authorization` header
- **THEN** the response is 401 with `detail` = `missing_token` and `WWW-Authenticate: Bearer`

#### Scenario: Wrong issuer or audience
- **WHEN** the token's `iss` differs from the configured issuer, or its `aud` does not contain the configured audience
- **THEN** the response is 401 with `detail` = `invalid_token` and `WWW-Authenticate: Bearer error="invalid_token"`

#### Scenario: Expired token
- **WHEN** the token's `exp` is further in the past than the leeway
- **THEN** the response is 401 with `detail` = `token_expired` and `WWW-Authenticate: Bearer error="invalid_token"`

#### Scenario: Token not yet valid
- **WHEN** the token's `nbf` or `iat` is further in the future than the leeway
- **THEN** the response is 401 with `detail` = `invalid_token`

#### Scenario: Disallowed or mismatched algorithm
- **WHEN** the token's header names `none`, an HMAC algorithm, or an algorithm that does not fit the key its `kid` names
- **THEN** the response is 401 with `detail` = `invalid_token`

#### Scenario: Unknown signing key
- **WHEN** the token's `kid` is not in the identity provider's JWKS, even after a refresh
- **THEN** the response is 401 with `detail` = `invalid_token`

### Requirement: Signing key cache
The server SHALL fetch the identity provider's signing keys on first use and
cache them for the JWKS response's `Cache-Control: max-age` (one hour when
absent, at most 24 hours). A token whose `kid` is not cached SHALL trigger a
refresh. While keys are cached, refreshes SHALL start at most once every 30
seconds, and concurrent requests SHALL share a single refresh. A failed
refresh SHALL keep the last good keys. When no keys have ever been fetched,
a scoped endpoint SHALL answer 503 as `application/problem+json`.

#### Scenario: Key rotation
- **WHEN** the identity provider publishes a new key and a client presents a token signed with it more than 30 seconds after the last fetch
- **THEN** the server refreshes the keys once and accepts the token

#### Scenario: Refresh rate limit
- **WHEN** tokens with unknown `kid`s arrive within 30 seconds of the last fetch
- **THEN** the server does not fetch the JWKS again and refuses them with 401

#### Scenario: Identity provider down after keys were cached
- **WHEN** the cached keys expire and the refresh fails
- **THEN** tokens signed with a cached key are still accepted

#### Scenario: Identity provider never reachable
- **WHEN** no keys have ever been fetched and the JWKS cannot be fetched
- **THEN** the response is 503 as `application/problem+json`

### Requirement: Principal
A handler behind a scope SHALL receive the caller as a principal with these
fields:
- `subject`: the token's `sub`, or `azp` for `client_credentials` tokens, which carry no `sub`;
- `kind`: user or client;
- `client_id`: `azp`;
- `scopes`: from the space-separated `scope` claim;
- `workspace`: from a `workspace_id` claim at the token root or under `custom`.

A token with neither `sub` nor `azp`, or with two different workspace ids,
SHALL be refused as invalid.

#### Scenario: Client credentials token
- **WHEN** a valid token has no `sub` and `azp` = `acme-sync`
- **THEN** the principal's subject and client id are `acme-sync` and its kind is client

#### Scenario: Workspace claim
- **WHEN** a valid token carries `workspace_id` at its root, or under `custom`
- **THEN** the principal's workspace is that value

### Requirement: Scope per endpoint
Every protected endpoint SHALL require exactly one scope from the catalogue.
A valid token that does not grant that scope SHALL be refused with 403 as
`application/problem+json`, `detail` = `insufficient_scope`, and
`WWW-Authenticate: Bearer error="insufficient_scope", scope="<scope>"`.
Whether the caller may see a particular record SHALL be decided by the owning
module, and a read the caller may not see SHALL answer 404, not 403.

#### Scenario: Missing scope
- **WHEN** a client calls an endpoint requiring `x:read` with a valid token whose `scope` lacks `x:read`
- **THEN** the response is 403 with `WWW-Authenticate: Bearer error="insufficient_scope", scope="x:read"`

### Requirement: Scope catalogue
Each module SHALL declare its scopes once, named `<module>:<action>`, each
with a description. The server SHALL publish the catalogue of all modules'
scopes through `campbooks-server scopes` as a JSON array of
`{name, description}` sorted by name. The `system` module SHALL declare
`system:read` ("Read operational status.").

#### Scenario: Printing the catalogue
- **WHEN** an operator runs `campbooks-server scopes`
- **THEN** it prints a JSON array that includes `{"name": "system:read", "description": "Read operational status."}` and exits 0

#### Scenario: Misnamed scope
- **WHEN** a module declares a scope that does not start with its own name followed by `:`
- **THEN** `cargo xtask arch` fails naming the scope and the module

### Requirement: Documented access rules
The OpenAPI document SHALL define a `bearer` security scheme (HTTP bearer,
JWT) whose description lists the scope catalogue. Every operation SHALL state
its security explicitly: either the `bearer` scheme with its required scope,
or an empty requirement for a public endpoint. `GET /api/v1/system/health`
and `GET /api/v1/system/version` SHALL be documented as public.
`cargo xtask arch` SHALL fail when an operation states no security, names an
unknown scheme or a scope no module declares, or requires a bearer token
without a scope.

#### Scenario: Public endpoints
- **WHEN** a client reads `/openapi.json`
- **THEN** the health and version operations carry `"security": [{}]`

#### Scenario: Operation without security
- **WHEN** a handler's `#[utoipa::path]` has no `security` attribute
- **THEN** `cargo xtask arch` fails naming the operation's method and path
