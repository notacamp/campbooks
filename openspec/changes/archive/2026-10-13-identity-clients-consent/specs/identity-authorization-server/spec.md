## REMOVED Requirements

### Requirement: First-party clients without provisioning
**Reason**: the native client is no longer off: it is on, and asked for on the consent screen every time (slice S4).
**Migration**: see "First-party clients, the native one always asked"; nothing to do for installs.

## MODIFIED Requirements

### Requirement: Issuer metadata
The issuer SHALL be the instance's public origin. It SHALL publish OpenID
Connect Discovery and RFC 8414 metadata naming its endpoints (with
`registration_endpoint` only while dynamic registration is on), `S256` as
the only code challenge method, the
grant types `authorization_code`, `refresh_token` and `client_credentials`,
`EdDSA` as the signing algorithm, the scopes it accepts, support for the
RFC 9207 `iss` response parameter, and
`client_id_metadata_document_supported` (true only while metadata documents
are on).

#### Scenario: Metadata advertises PKCE
- **WHEN** a client fetches `/.well-known/oauth-authorization-server`
- **THEN** `issuer` is the public origin and `code_challenge_methods_supported` is `["S256"]`

#### Scenario: Metadata names registration
- **WHEN** a client fetches `/.well-known/oauth-authorization-server`
- **THEN** with dynamic registration on, `registration_endpoint` is `<origin>/oauth2/register`; with it off (the default) there is none; `grant_types_supported` includes `client_credentials`

### Requirement: Authorization code with PKCE
`/oauth2/authorize` SHALL accept only `response_type=code` with an S256
`code_challenge`, and only scopes the client may ask for: the OIDC scopes,
and for Campbooks' own clients every scope of the catalogue, for any other
client the ones it registered and never an `identity:*` scope,
`workspaces:write`, `workspaces:admin` or `admin:access`. It SHALL
match `redirect_uri` exactly against the client's registered values, except
that a loopback redirect matches on any port (RFC 8252 §7.3). For an unknown
client or an unregistered redirect it SHALL answer 400 and never redirect.
Every other error, and every code, SHALL go back to the client's redirect
URI with `state` and `iss`. A browser already signed in to the issuer SHALL
get a code at once for the web client, or go through the consent rules
below for any other client, unless the client sends `prompt=login` or a
`max_age` it exceeds; with `prompt=none` and no session the client SHALL get
`error=login_required`, and with consent to ask `error=consent_required`.
Codes SHALL be single-use and expire after 60 seconds; redeeming one again
SHALL fail and revoke every token issued from it.

#### Scenario: Plain PKCE refused
- **WHEN** a client sends `code_challenge_method=plain`
- **THEN** the browser is redirected back with `error=invalid_request`, `state` and `iss`, and no code

#### Scenario: Unregistered redirect
- **WHEN** a request names a `redirect_uri` the client did not register
- **THEN** the response is 400 with `code` = `invalid_redirect_uri` and no `Location`

#### Scenario: Code replay
- **WHEN** a code is redeemed a second time
- **THEN** the token request fails with `invalid_grant` and the refresh token issued at the first redemption stops working

#### Scenario: A third party asks for an identity scope
- **WHEN** a dynamically registered client asks for `identity:write`
- **THEN** the browser is redirected back with `error=invalid_scope`

### Requirement: Access and ID tokens
Access tokens SHALL be JWTs with header `typ` = `at+jwt`, signed with an
Ed25519 key published in `/oauth2/jwks`, carrying `iss`, `sub`, `aud` (the
API's resource URI, `<public origin>/api`), `client_id`, `azp`, `scope`,
`iat`, `nbf`, `exp` (ten minutes after `iat`), `jti`, `auth_time` for a
person's token, `workspace_id` when the token is bound to a workspace, and
the person's address and name when `email` and `profile` were granted. ID
tokens SHALL be signed the same way with `typ` = `JWT`, for the client,
carrying the request's `nonce`. The API SHALL validate access tokens with the
same verifier it uses for any issuer, reading the keys in-process, SHALL
expect the resource URI as `aud`, and SHALL accept only `typ` = `at+jwt`
from the in-process issuer, so an ID token never passes as an access token.

#### Scenario: Token accepted by the API
- **WHEN** the web client calls the API with an access token from the issuer
- **THEN** the API accepts it and the handler sees the account id as the principal's subject

#### Scenario: An ID token is not an access token
- **WHEN** a client calls the API with an ID token of the in-process issuer whose `aud` matches the API's
- **THEN** the response is 401 with `code` = `invalid_token`

## ADDED Requirements

### Requirement: First-party clients, the native one always asked
The web client `campbooks-web` (confidential, its secret derived from the
instance secret key) and the native client `campbooks-native` (public,
PKCE, redirects `http://127.0.0.1:<any port>/callback`,
`http://[::1]:<any port>/callback` and
`com.notacamp.campbooks:/oauth/callback`) SHALL exist on every install with
no provisioning step. The web client SHALL get no consent screen. The native
client SHALL get a code only after the person allows it on the consent
screen, for every authorization: its consent SHALL never be remembered,
because any process on a device can listen on a loopback port or claim a
custom scheme (RFC 8252 §8.6). Its tokens SHALL carry no `workspace_id`.

#### Scenario: The native client asks every time
- **WHEN** a signed-in browser is sent to `/oauth2/authorize` for `campbooks-native` with a loopback redirect, twice
- **THEN** both times the browser goes to `/consent?request=`, and no code reaches the redirect until the person presses Allow

#### Scenario: The native client's token
- **WHEN** the person allows the native client and it redeems the code with its PKCE verifier and no secret
- **THEN** it gets an access token whose `aud` is the API's resource URI and that carries no `workspace_id`, and a refresh token


### Requirement: Consent for every client but the web app
Before a client other than `campbooks-web` gets a code, the person SHALL
allow it on the consent screen, unless a remembered consent covers the
request. A consent SHALL be remembered only when the client's redirect is an
exact `https` URI; it SHALL cover later requests to that same redirect URI,
for at most the scopes allowed, for 90 days, and SHALL bind them to the same
workspace; `prompt=consent` SHALL always show the screen. Every code and
refresh token of such a client SHALL stand on a live consent of the person
for the same workspace and covering its scopes, checked when it is redeemed
(`invalid_grant` otherwise); a new consent that replaces one SHALL end the
client's refresh tokens bound to another workspace or to more scopes. The
screen SHALL say "verified" for Campbooks' own apps only, and name the
workspace for a workspace's own app. When the screen is due and the person's sign-in is
older than ten minutes, the issuer SHALL send them to sign in again first.
`GET /auth/consent/{request}` SHALL answer only the browser signed in as the
person the request was put to (404 otherwise) with the client (name, kind,
whether Campbooks or a workspace admin registered it, the redirect host,
whether the redirect is on the device), the scopes with descriptions, the
person's workspaces it may use, and whether the answer will be remembered.
`POST /auth/consent/{request}` SHALL need the `X-Campbooks-CSRF` check, the
same person's issuer session, a sign-in in the last ten minutes for Allow
(`step_up_required`), and a workspace the person belongs to for a client
that binds one (404 otherwise; `consent_needs_workspace` when missing); a
workspace's own client SHALL be allowed in its workspace only
(`consent_wrong_workspace`). Allow SHALL record the consent, publish
`identity.consent_granted` and answer the client's redirect with a code bound
to that workspace; Deny SHALL answer it with `error=access_denied`. Either
SHALL spend the request.

#### Scenario: Remembered for an https redirect
- **WHEN** a person allows a client with an `https` redirect in workspace Home and it asks again for the same scopes
- **THEN** the second authorization returns a code at once, bound to Home

#### Scenario: Never remembered for a loopback redirect
- **WHEN** the same client asks through a loopback redirect
- **THEN** the browser goes to the consent screen again

#### Scenario: Another person's request
- **WHEN** a browser signed in as someone else opens or answers the request
- **THEN** both answer 404

#### Scenario: Forged answer
- **WHEN** a POST to `/auth/consent/{request}` lacks the CSRF header
- **THEN** the response is 403 `csrf_check_failed` and nothing is allowed

### Requirement: Workspace-bound tokens
A token issued after a consent in a workspace SHALL carry that workspace as
`workspace_id`. The in-process verifier SHALL keep that claim on a person's
token (an outside identity provider's user claims stay ignored), and
`Workspace<S>` SHALL answer 404 for any other workspace, whatever the
scopes; membership SHALL still be checked. Outside a workspace, such a token
SHALL list that workspace only (`workspaces.list`), and SHALL get 404 from
creating a workspace, accepting an invitation and every account-wide route
(`identity.*`).

#### Scenario: Another workspace
- **WHEN** an agent allowed in Home calls a Work route with its token
- **THEN** the response is 404, while the same route of Home answers

### Requirement: Connected apps
`identity.list_consents` (`identity:read`) SHALL list the caller's live
consents with the client's name, kind, whether it is verified, the bound
workspace, the scopes and the dates; `identity.revoke_consent`
(`identity:write`) SHALL revoke one and every refresh token the client holds
for the caller, publish `identity.consent_revoked`, and answer 404 for a
consent that is not the caller's or already revoked. When a member leaves a
workspace, the consents they gave in it SHALL be revoked; when a workspace is
being deleted, its clients SHALL be removed and every consent bound to it
revoked.

#### Scenario: Revoke
- **WHEN** a person revokes an agent's consent and the agent refreshes its token
- **THEN** the refresh fails with `invalid_grant` and the next authorization shows the consent screen

### Requirement: A workspace's own clients
Owners and admins SHALL list, register, rotate the secret of and remove the
workspace's clients under `/api/v1/w/{workspace_id}/identity/clients`
(`identity:clients`; anyone else 403 `not_workspace_admin`). Registering and
rotating SHALL need a sign-in in the last ten minutes. A confidential client
SHALL get a 256-bit random secret, returned only in that answer and stored as
its SHA-256 only, and only the `client_credentials` grant; a public client
SHALL get code + PKCE with 1 to 10 `https` or loopback redirect URIs. A
client SHALL never be given a scope a third party may not hold
(`client_scope_not_allowed`), nor a name that reads as Campbooks or Not A
Camp (`client_name_reserved`).
Rotating SHALL stop the old secret at once; removing SHALL end the client's
consents and refresh tokens. The table SHALL be under forced row-level
security.

#### Scenario: Secret shown once
- **WHEN** an owner registers a confidential client and then lists the workspace's clients
- **THEN** the registration answer holds the secret and the list never does

#### Scenario: Rotation
- **WHEN** an owner rotates a client's secret
- **THEN** `client_credentials` with the old secret answers 401 `invalid_client` and the new one works

### Requirement: Client credentials
`/oauth2/token` with `grant_type=client_credentials` SHALL be accepted from a
workspace's confidential client only (anyone else `unauthorized_client`),
authenticated with `client_secret_basic` or `client_secret_post`, for at most
the scopes it was given (`invalid_scope`). Its access token SHALL have `sub` =
`azp` = the client id and `workspace_id` = its workspace, and come with no
refresh token and no ID token.

#### Scenario: A workspace's export
- **WHEN** a workspace's confidential client asks for a token with its secret
- **THEN** the token is bound to its workspace and the API answers its workspace's routes and 404 for another's

### Requirement: Resource indicators and protected-resource metadata
The API's resource identifier SHALL be `<public origin>/api`. A `resource`
other than it, at `/oauth2/authorize` or `/oauth2/token`, SHALL be refused
with `invalid_target`. The issuer SHALL serve RFC 9728 metadata for it at
`/.well-known/oauth-protected-resource/api` and
`/.well-known/oauth-protected-resource` (`resource`,
`authorization_servers`, the scopes a third party may hold,
`bearer_methods_supported`), and every Bearer challenge of the API (401, and
403 `insufficient_scope`) SHALL carry
`resource_metadata="<origin>/.well-known/oauth-protected-resource/api"`.

#### Scenario: An agent finds the issuer
- **WHEN** an agent calls the API without a token
- **THEN** the 401 says `Bearer resource_metadata="<origin>/.well-known/oauth-protected-resource/api"`, and that document names the issuer

#### Scenario: Another resource
- **WHEN** a client asks for a token for `https://other.example.com/api`
- **THEN** the answer is `invalid_target`

### Requirement: Dynamic client registration
Dynamic registration SHALL be off unless
`CAMPBOOKS_OAUTH__DYNAMIC_REGISTRATION` is on: then `/oauth2/register`
answers 404 and clients registered while it was on are unknown. When on,
`POST /oauth2/register` (RFC 7591) SHALL register public clients only: 1 to
10 redirect URIs, each `https` or loopback `http` without fragment or
userinfo (`invalid_redirect_uri`); `token_endpoint_auth_method` `none` or
absent, grant types within `authorization_code` and `refresh_token`,
response types within `code` (`invalid_client_metadata`). It SHALL drop
any scope a third party may not hold, and with no `scope` grant read scopes
only. It SHALL clean the name of control, format and direction characters,
and SHALL show the redirect host instead of a name that reads as Campbooks or
Not A Camp. It SHALL issue no secret and no registration access token,
limit registrations per client address and for the whole instance per hour
(429 `rate_limited`), and delete a client unused for 30 days. Such clients SHALL be shown on the consent screen as
unverified.

#### Scenario: Off by default
- **WHEN** an instance that did not set `CAMPBOOKS_OAUTH__DYNAMIC_REGISTRATION` receives a registration
- **THEN** the answer is 404, and its issuer metadata has no `registration_endpoint`

#### Scenario: A confidential registration
- **WHEN** a client registers with `token_endpoint_auth_method=client_secret_basic`
- **THEN** the answer is 400 `invalid_client_metadata` and nothing is stored

#### Scenario: Too many registrations
- **WHEN** one address registers a 21st client within the hour
- **THEN** the answer is 429 with `Retry-After`

### Requirement: Client ID Metadata Documents
Metadata documents SHALL be off unless
`CAMPBOOKS_OAUTH__CLIENT_METADATA_DOCUMENTS` is on: then an `https`
`client_id` is an unknown client and nothing is fetched. When on, an `https`
`client_id` URL with a path SHALL name a client whose metadata
document is fetched at that URL through an SSRF guard: `https` only, no IP
literal, no userinfo, the default port; every address the name resolves to
public (no loopback, private, link-local or metadata, CGNAT, multicast,
documentation, reserved, unique-local or site-local, nor their IPv4-mapped or
NAT64 forms) and the request sent to the checked address; no redirects, 5 s,
64 KiB, `application/json` only. The document's `client_id` SHALL equal the
URL, its redirect URIs SHALL follow the registration rules, and it SHALL not
claim a secret or keys; anything else SHALL make the request `invalid_client`
(shown, never redirected). A valid document SHALL be cached for its
`Cache-Control: max-age`, between 5 minutes and 24 hours; a failure SHALL
not be fetched again for 5 minutes, and the instance SHALL fetch at most a
fixed number of documents per hour. No proxy SHALL be used, and IPv4-
translated addresses (`::ffff:0:0:0/96`) SHALL be refused too.

#### Scenario: A document that lies
- **WHEN** a client id URL serves a document whose `client_id` is another URL
- **THEN** `/oauth2/authorize` answers 400 `invalid_client` with no `Location`

#### Scenario: Cached
- **WHEN** the same metadata-document client asks twice within its `max-age`
- **THEN** the document is fetched once
