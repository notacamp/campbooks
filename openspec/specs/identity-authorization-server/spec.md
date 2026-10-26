# identity-authorization-server Specification

## Purpose
Campbooks as the OAuth 2.1 authorization server and OpenID provider of its
own API, on the app's origin, with keys it generates and rotates itself
(ADR 0014).

## Requirements

### Requirement: When the issuer runs in this process
The issuer SHALL run in the server when the instance has a secret key and
`CAMPBOOKS_AUTH__ISSUER` is unset or names `CAMPBOOKS_PUBLIC_URL`'s origin.
Otherwise every identity endpoint SHALL answer 404, and the configured
identity provider SHALL keep serving the instance. Outside
`CAMPBOOKS_ENV=development` the server SHALL refuse to start the issuer on a
plain `http` public URL, so its cookies are always `Secure` and
`__Host-`-prefixed.

#### Scenario: Another identity provider is configured
- **WHEN** `CAMPBOOKS_AUTH__ISSUER` names another identity provider and a client requests `/.well-known/openid-configuration`
- **THEN** the response is 404

#### Scenario: Plain http in production
- **WHEN** the issuer would run with `CAMPBOOKS_PUBLIC_URL=http://app.example.com` and `CAMPBOOKS_ENV=production`
- **THEN** the server refuses to start, naming `CAMPBOOKS_PUBLIC_URL`

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

### Requirement: Client authentication at the token endpoint
Confidential clients SHALL authenticate with `client_secret_basic` or
`client_secret_post`, compared in constant time; public clients SHALL send
only `client_id`. A failure SHALL answer 401 `invalid_client`. Token
responses SHALL be `Cache-Control: no-store`.

#### Scenario: Wrong secret
- **WHEN** the web client redeems a code with a wrong secret
- **THEN** the response is 401 with `error` = `invalid_client`

### Requirement: Refresh-token rotation with reuse detection
A refresh token SHALL be issued when `offline_access` was granted. Every
refresh SHALL return a new refresh token and invalidate the one used.
Presenting a rotated refresh token SHALL fail with `invalid_grant`, revoke
every token of its sign-in, and record `identity.refresh_reuse_detected`.

#### Scenario: A replayed refresh token
- **WHEN** refresh token R1 is used to obtain R2 and then R1 is presented again
- **THEN** the request fails with `invalid_grant` and R2 no longer works

### Requirement: Access and ID tokens
Access tokens SHALL be JWTs with header `typ` = `at+jwt`, signed with an
Ed25519 key published in `/oauth2/jwks`, carrying `iss`, `sub`, `aud` (the
API's resource URI, `<public origin>/api`), `client_id`, `azp`, `scope`,
`iat`, `nbf`, `exp` (ten minutes after `iat`), `jti`, `auth_time` for a
person's token, `sid` (the issuer session) when it was issued in one, `fam`
(the refresh family) when its sign-in holds a refresh token, kept across
rotation, `cns` (the consent it was minted under) for an app that needs one,
`workspace_id` when the token is bound to a workspace, and
the person's address and name when `email` and `profile` were granted. ID
tokens SHALL be signed the same way with `typ` = `JWT`, for the client,
carrying the request's `nonce` and the session's authentication methods
(`amr`, RFC 8176): `["fed"]` for a provider sign-in, `["hwk", "user"]` for a
device-bound passkey and `["swk", "user"]` for a synced one. The API SHALL validate access tokens with the
same verifier it uses for any issuer, reading the keys in-process, SHALL
expect the resource URI as `aud`, and SHALL accept only `typ` = `at+jwt`
from the in-process issuer, so an ID token never passes as an access token.

#### Scenario: Token accepted by the API
- **WHEN** the web client calls the API with an access token from the issuer
- **THEN** the API accepts it and the handler sees the account id as the principal's subject

#### Scenario: An ID token is not an access token
- **WHEN** a client calls the API with an ID token of the in-process issuer whose `aud` matches the API's
- **THEN** the response is 401 with `code` = `invalid_token`

#### Scenario: A passkey sign-in's amr
- **WHEN** the web client redeems a code from a sign-in with a device-bound passkey
- **THEN** the ID token's `amr` is `["hwk", "user"]`, while one from a Google sign-in has `["fed"]`

### Requirement: Signing-key lifecycle
The server SHALL create a signing key the first time it needs one, exactly
once however many processes start together. It SHALL store private keys
sealed with a key derived from the instance secret key, publish the next key
seven days before it signs, rotate the signing key every 30 days, and keep a
retired key published for two days. When the signing key cannot be opened
with the instance secret key, the issuer SHALL fail closed: it SHALL sign
nothing, delete nothing, and the server SHALL refuse to start with an error
naming `CAMPBOOKS_SECRET_KEY`. No manual step SHALL be needed at install or
upgrade.

#### Scenario: Rotation without downtime
- **WHEN** 30 days pass after the first key was made
- **THEN** its successor, published for the past week, signs new tokens, and the first key stays in the JWKS for two more days

#### Scenario: Concurrent first start
- **WHEN** three processes ask for a signing key on an empty database at once
- **THEN** exactly one key exists and all three sign with it

#### Scenario: Another secret key
- **WHEN** a server starts with a `CAMPBOOKS_SECRET_KEY` other than the one the signing keys were sealed with
- **THEN** it refuses to start with an error naming `CAMPBOOKS_SECRET_KEY`, and the stored keys are untouched

### Requirement: The web BFF signs in against the in-process issuer
When the issuer runs in this process, the web BFF SHALL sign browsers in
against it as `campbooks-web`, with the derived secret and with its
discovery, token, revocation and end-session requests sent through the
issuer's router rather than the network. `/bff/logout` SHALL end the issuer
session itself, with the ID token it keeps as `id_token_hint`, and SHALL
give the browser no logout URL to visit.

#### Scenario: Web sign-in needs no provisioned secret
- **WHEN** a fresh install starts with `CAMPBOOKS_SECRET_KEY` and `CAMPBOOKS_PUBLIC_URL` only, and a person signs up with Google through `/bff/login`
- **THEN** they land in the app with a web session, and the BFF's API calls carry tokens the API accepts

#### Scenario: Signing out of the web app
- **WHEN** a signed-in person signs out of the web app and the app starts a new sign-in
- **THEN** `/bff/logout` answers no `logout_url` and the issuer shows its sign-in screen again

### Requirement: Revocation and logout
`/oauth2/revoke` SHALL revoke the sign-in a refresh token belongs to, for the
client that holds it, and SHALL answer 200 for unknown tokens (RFC 7009).
`/oauth2/end_session` (GET or form POST) SHALL require an `id_token_hint`
this issuer signed for one of Campbooks' own clients or an instance app
(expired is fine); without one it SHALL answer 400 `invalid_id_token_hint`
and end nothing. With one it SHALL end the issuer session the hint names and
that sign-in's refresh tokens, and redirect only to a
`post_logout_redirect_uri` the hint's client registered, matched exactly,
with `state` appended.

#### Scenario: Revoked refresh token
- **WHEN** a client revokes its refresh token and then uses it
- **THEN** the refresh fails with `invalid_grant`

#### Scenario: Cross-site sign-out refused
- **WHEN** another site sends a browser to `/oauth2/end_session` without a valid `id_token_hint`
- **THEN** the response is 400 with `code` = `invalid_id_token_hint` and the person's tokens keep working

#### Scenario: An instance app signs out
- **WHEN** Applicamp sends the browser to `/oauth2/end_session` with its ID token as the hint and its registered post-logout URI
- **THEN** the browser goes back to that URI with `state`, the sign-in's refresh tokens stop working, and another post-logout URI ends the session without a redirect

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
(`identity.*`), whatever scopes it holds. Inside its own workspace it SHALL
get 404 from managing the workspace's apps and adopting a held mailbox, and
the admin API SHALL answer it 404.

#### Scenario: Another workspace
- **WHEN** an agent allowed in Home calls a Work route with its token
- **THEN** the response is 404, while the same route of Home answers

#### Scenario: A bound token with identity scopes
- **WHEN** a token bound to Home that holds `identity:write` and `identity:clients` calls sign out everywhere, removes a way in, or lists Home's apps
- **THEN** each answers 404 and nothing changes

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
the scopes it was given and a third party may hold, checked again at issue
(`invalid_scope`). Its access token SHALL have `sub` = `azp` = the client id
and `workspace_id` = its workspace, and come with no refresh token and no ID
token.

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

### Requirement: Authorization requests belong to their browser
`/oauth2/authorize` SHALL bind every request it stores to the browser that
made it (a random `__Host-campbooks_browser` cookie, `SameSite=Lax`, stored
as its SHA-256). Showing a request to the sign-in screen, using it in a
passkey ceremony, completing it after a sign-in and answering its consent
SHALL need that browser's cookie; in another browser the request SHALL be
unknown (404, or the sign-in lands on its plain return path with no code).

#### Scenario: A request id sent to someone else
- **WHEN** a sign-in screen link with a waiting request is opened in another browser and the person there signs in
- **THEN** the request is 404 there, no code is issued to its client, and the request still waits for its own browser

### Requirement: Refresh never outlives its sign-in
A refresh family SHALL expire no later than the issuer session it was issued
in.

#### Scenario: A sign-in near its end
- **WHEN** a code is redeemed from an issuer session that ends in an hour
- **THEN** its refresh token stops working when that session ends

### Requirement: Issuer endpoints are limited per address
`/oauth2/authorize` SHALL answer at most 60 requests per client address a
minute, `/oauth2/end_session` 30 and the web BFF's `/bff/login` 30; beyond
that 429 `rate_limited`. Calls made inside the server SHALL not be counted.

#### Scenario: Hammering authorize
- **WHEN** one address sends a 61st authorization request within a minute
- **THEN** the answer is 429, while another address is still served

### Requirement: A copied web session ends
The web BFF SHALL accept a rotated-away session cookie only during its
two-minute grace. Presented later, however many rotations later while the
session lives, once its successor has been used, the session SHALL end
everywhere it reaches: the web session, its refresh token at the issuer and
the issuer session; the log line SHALL name no session or person. While the
successor has never been presented (the response carrying it was lost), the
old cookie SHALL instead get a fresh one, and the undelivered successor SHALL
count as rotated away.

#### Scenario: An old cookie comes back
- **WHEN** a session's cookie rotated three minutes ago and the old value is presented
- **THEN** neither browser is signed in any more and the session's refresh token is revoked

#### Scenario: The new cookie never arrived
- **WHEN** a rotation's response is lost and the browser presents its old cookie three minutes later
- **THEN** it stays signed in with a fresh cookie, nothing is revoked, and the lost cookie, presented later, ends the session

#### Scenario: A waking page fires several requests
- **WHEN** two requests present the same old cookie at once after a lost rotation
- **THEN** one gets a fresh cookie, the other goes through within the grace, and the session stays signed in

#### Scenario: A copy two rotations old
- **WHEN** a session rotated twice while in use and a copy of its first cookie is presented
- **THEN** the session ends the same way

### Requirement: Names that read as Campbooks
Before deciding whether an app's name reads as Campbooks or Not A Camp, the
server SHALL decompose it for compatibility (NFKD), drop every mark
(general category M, in any script), reduce it to its UTS #39 confusables
skeleton and fold case, spaces and punctuation, so full-width, mathematical,
circled, accented and other scripts' look-alike variants count too; any
other letter, digit or symbol outside ASCII nothing maps SHALL count as any
character, but a match SHALL rest on at most two of them, so a name in
another script is not refused for its length alone. A word of an app's name
SHALL NOT mix scripts beyond UTS #39's "highly restrictive" level (400
`client_name_mixed_scripts` for a workspace's app; a self-registered agent
is shown under its redirect host).

#### Scenario: A full-width name
- **WHEN** a workspace registers an app named `Ｃａｍｐｂｏｏｋｓ` or `Cámpbooks`
- **THEN** the answer is 400 `client_name_reserved`

#### Scenario: Letters with no decomposition
- **WHEN** a workspace registers an app named `Campbøøks` or `ᴄᴀᴍᴘʙᴏᴏᴋꜱ`
- **THEN** the answer is 400 `client_name_reserved`

#### Scenario: Homoglyphs of other scripts
- **WHEN** a workspace registers an app named `Cɑmpbօօks`, `ᏟᎪᎷᏢᏴOOᏦᏚ` or `Campb○○ks`
- **THEN** it is refused (`client_name_reserved`)

#### Scenario: A word mixing scripts
- **WHEN** a workspace registers an app named `Pаyroll sync` with a Cyrillic `а`
- **THEN** the answer is 400 `client_name_mixed_scripts`

#### Scenario: Other scripts
- **WHEN** a workspace registers an app named `メールアシスタントプロ`, `José` or `Françoise`
- **THEN** it is registered under that name

### Requirement: The shipped self-host stack runs no separate identity provider
The shipped `deploy/compose.yaml` SHALL run no identity-provider service and SHALL NOT set `CAMPBOOKS_AUTH__ISSUER`, so a default install signs people in through the issuer in the server. Provider client secrets SHALL be files under `deploy/secrets/`, mounted read-only into the server.

#### Scenario: A fresh stack
- **WHEN** a self-hoster runs `docker compose up -d` with the shipped files and only the required settings
- **THEN** the only services are the server, Postgres and their one-shot helpers, and the server serves `/.well-known/openid-configuration` on its own origin

### Requirement: Revoked access tokens stop at once
Each account SHALL carry a revocation epoch (`tokens_not_before`), set to
the revocation's instant when the account is suspended or signs out
everywhere, in the same transaction, and never moved back (unsuspending
leaves it). The API SHALL check every person's access token of the
in-process issuer, on every verification, against the account through the
kernel's `TokenRevocation` port, and SHALL refuse it when the account does
not exist, is suspended, the token carries no `iat`, or its `iat` is not in
a later second than the epoch. A refused token SHALL get exactly the answer
of any invalid token (401, `code` `invalid_token`,
`WWW-Authenticate: Bearer error="invalid_token"`; 404 on the admin API),
saying nothing about the account. Every way a token is verified (scoped,
workspace and admin routes, the idempotency layer, the web BFF's proxied
calls) SHALL apply it. The token endpoint SHALL mint nothing
(`invalid_grant`) for a code or refresh token whose sign-in (`auth_time`)
is not newer than the epoch, and SHALL stamp a newer sign-in's access token
minted in the epoch's own second with the next second as `iat`, reading the
account under a share lock while minting. Client-credentials tokens and an
outside identity provider's tokens SHALL be verified as before.

#### Scenario: Suspended
- **WHEN** an account is suspended while a client holds one of its access tokens
- **THEN** that token gets 401 `invalid_token` on a scoped route and through the idempotency layer (a `POST` with an `Idempotency-Key`, replaying a key it used before), and 404 on the admin API

#### Scenario: Unsuspended
- **WHEN** the account is then unsuspended
- **THEN** the old token still gets 401, and a token from a new sign-in works

#### Scenario: Signed out everywhere
- **WHEN** a person signs out everywhere
- **THEN** the access tokens issued before get 401, and a token from a sign-in after it works

#### Scenario: The revocation's own second
- **WHEN** one token is minted earlier in the second of a revocation and another for a new sign-in later in that same second
- **THEN** the first is refused and the second is accepted

#### Scenario: A code from before the revocation
- **WHEN** a code handed out before a person signed out everywhere is redeemed afterwards
- **THEN** the token endpoint answers `invalid_grant`

#### Scenario: Not this issuer's people
- **WHEN** the API validates a workspace client's `client_credentials` token, or runs with an outside identity provider
- **THEN** no revocation is looked up and the token is accepted as before

### Requirement: One sign-in's access tokens stop at once
The API SHALL also refuse a person's access token of the in-process issuer,
on every verification and through the same `TokenRevocation` port, once its
own sign-in has ended: its issuer session (`sid`) ended (signed out, or
ended because the way in it used was removed), its refresh family (`fam`)
revoked (`/oauth2/revoke`, reuse, sign-out), or the consent it was minted
under (`cns`) revoked, each looked up by its id. The token gets the answer
of any invalid token. Every other sign-in of the same person SHALL keep
working. An app asked again for exactly what its live consent holds (the
same workspace, member and scopes) SHALL keep that consent, so its tokens on
the person's other devices go on; a different grant replaces it. The check
SHALL cost the single statement the account check already needs, with no
cache, so a revocation takes effect on the next request in every process. A
token from before these claims is judged by its account alone. A sign-in
named with something that is no id SHALL stand for nothing.

#### Scenario: One session signed out
- **WHEN** a person signed in in two browsers signs out of the first
- **THEN** the first browser's access token gets 401 `invalid_token` at once, and the second's still works and refreshes

#### Scenario: One refresh token revoked
- **WHEN** the first sign-in's refresh token is revoked at `/oauth2/revoke`
- **THEN** that sign-in's access token gets 401 at once, and the other sign-in's tokens, refreshed ones included, still work

#### Scenario: A way in removed
- **WHEN** a person removes the way in another browser signed in with
- **THEN** that browser's access token gets 401 at once, and the person's own sign-in still works

#### Scenario: An app's consent revoked
- **WHEN** a person revokes an app's consent
- **THEN** the app's access token gets 401 `invalid_token` at once, and the person's own tokens still work

#### Scenario: An app asked again
- **WHEN** the desktop app, asked for consent on every sign-in, is allowed again for the same scopes and its code redeemed at once
- **THEN** the new token works at once, and the token on the first device still works

#### Scenario: Granted again after a revocation
- **WHEN** an app's consent was revoked and the person allows it again
- **THEN** the token minted under the new consent works at once

### Requirement: Instance apps exist only through the server CLI
An instance app is a confidential sign-in client of the whole instance (no
workspace), for the operator's own other apps. It SHALL be registered,
listed, given a new secret and removed only by `campbooks-server admin apps
add|list|rotate-secret|remove`, on the migration login; no HTTP endpoint
SHALL do any of these, and the runtime role SHALL be refused any insert,
update or delete of an instance app except recording when it last got a
token. Registering SHALL print its client id (`app_` followed by 16 random
bytes in unpadded base64url) and a 256-bit random secret, shown this once
and stored as its SHA-256 only; rotating SHALL print a new secret once and
stop the old one at once; removing SHALL revoke every consent people gave
it and every refresh token it holds. Registering SHALL refuse: a name that
is empty, longer than 100 characters, reads as Campbooks or Not A Camp, or
mixes scripts in a word; 0 or more than 10 redirect URIs, or more than 10
post-logout redirect URIs; a URI that is not `https` with a host (loopback
`http` only when the instance runs in development), has a fragment or
userinfo, is longer than 2048 characters, is not in its canonical form, or
is given twice; a scope ceiling that is empty or holds anything but
`openid`, `profile`, `email`, `offline_access` and the API scopes a third
party may hold. Each change SHALL be recorded in the identity audit log
(`instance_app_registered`, `instance_app_secret_rotated`,
`instance_app_removed`), with ids and counts only.

#### Scenario: Register, list, rotate, remove
- **WHEN** an operator registers Applicamp, lists the instance apps, rotates its secret and removes it
- **THEN** the registration prints its client id and secret, the list shows its name, scopes and redirect URIs and never a secret, the old secret answers 401 `invalid_client` after the rotation while the new one works, and after the removal the app is unknown to the token endpoint

#### Scenario: The secret is kept hashed
- **WHEN** an instance app is registered
- **THEN** the stored `secret_hash` is the secret's SHA-256 and the secret itself is stored nowhere

#### Scenario: A ceiling above what a third party may hold
- **WHEN** an operator registers an app with `identity:read`, `workspaces:write`, `admin:access` or an unknown scope in its ceiling
- **THEN** the command refuses it and nothing is stored

#### Scenario: A plain http redirect
- **WHEN** an operator registers an app with `http://applicamp.example.com/cb`, or with `http://127.0.0.1:4000/cb` on an instance not in development
- **THEN** the command refuses it

#### Scenario: The runtime role cannot change one
- **WHEN** a statement on the runtime role inserts an instance app, widens one's ceiling, adds a redirect URI, sets its secret, changes its kind or deletes it
- **THEN** the database refuses it, while recording its last use succeeds

### Requirement: Instance apps sign people in
An instance app SHALL use the authorization code with PKCE (S256, required
as for every client) and authenticate at the token endpoint with its secret
(`client_secret_basic` or `client_secret_post`); without it or with another
the answer SHALL be 401 `invalid_client`. Its redirect URIs SHALL match
exactly, with no loopback port leeway. An authorization SHALL be granted the
scopes it asks for that are in the app's ceiling and that a third party may
hold, and no other; when none is left the answer SHALL be `invalid_scope`.
Its tokens SHALL be workspace-bound and stand on a consent like any other
third-party app's: the consent screen once per person for the exact `https`
redirect (remembered as for any client), the workspace the person picks,
listed in Connected apps and revocable there. Its ID token SHALL carry
`sub`, `email`, `email_verified`, `name` (with `profile` and `email`) and
the request's `nonce`; with `offline_access` it SHALL get a refresh token,
rotated on every use with reuse detection. The consent screen SHALL show it
with kind `instance`, as added by this server, not as unverified, and not
as Campbooks' own (`verified` false).

#### Scenario: Code, PKCE and the secret
- **WHEN** a person allows Applicamp in Home and Applicamp redeems the code with its PKCE verifier and its secret by HTTP Basic
- **THEN** it gets an access token bound to Home, a refresh token and an ID token with the person's `sub`, `email`, `email_verified`, `name` and the `nonce`

#### Scenario: Without the verifier or the secret
- **WHEN** Applicamp redeems a code without its PKCE verifier, or without its secret
- **THEN** the answers are `invalid_request` and 401 `invalid_client`, and the code still works afterwards with both

#### Scenario: A redirect URI it did not register
- **WHEN** an authorization names Applicamp's callback with a trailing slash, a query, another port, another host, or a loopback URI
- **THEN** the answer is 400 `invalid_redirect_uri` and nothing is redirected

#### Scenario: Asking above the ceiling
- **WHEN** Applicamp, whose ceiling lacks `system:read`, asks for `openid … system:read`
- **THEN** the consent screen and the token list every scope asked for but `system:read`; asking only for scopes above the ceiling answers `invalid_scope`

#### Scenario: Asked once, then remembered
- **WHEN** a person allows Applicamp in Work and Applicamp asks again for the same scopes
- **THEN** the second authorization returns a code at once, bound to Work

#### Scenario: Revoked, then asked again
- **WHEN** the person revokes Applicamp in Connected apps
- **THEN** its access token answers 401, its refresh token `invalid_grant`, and its next authorization shows the consent screen

#### Scenario: A removed app's tokens
- **WHEN** an operator removes Applicamp
- **THEN** its refresh tokens answer 401 `invalid_client` at the token endpoint and its access tokens answer 401 at once
