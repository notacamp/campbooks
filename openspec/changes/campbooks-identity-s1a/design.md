# Design

## Context

ADR 0014 records the decision. This document is the how: who owns what, the
data, the flows, the alternatives behind each choice, the security posture,
the migration away from Rauthy, and the order of the pull requests. It
records every decision of ADR 0014, because they were taken together; this
change builds **slice S1a** (see "This change: slice S1a" below), and each
later slice writes and archives its own change for its part.

What existed on `next` before this change:

- **`kernel::auth`** validates JWT access tokens: an EdDSA/RS256 allowlist,
  issuer, audience and expiry, with keys from a lazily fetched, single-flight,
  rotation-aware JWKS cache. It also validates ID tokens and logout tokens
  for the BFF. `kernel::testing::TestIdp` fakes an issuer in tests.
- **The BFF** (`crates/server/src/bff`, schema `bff`) is a confidential OIDC
  client of Rauthy. It keeps tokens encrypted server-side and gives the
  browser one `__Host-` cookie. `/bff/api/*` runs requests through the API
  router in-process.
- **`workspaces`** keys `users` by `idp_subject`. It applies the signup
  policy when a workspace is created, and requires a verified email matching
  the invitation to accept one.
- **`connectors`** (#450) has its own Google consent flow
  (`start_connection`/`complete_connection`). That flow funnels into one
  seam, `domain::grants::create_or_refresh_connection(ProviderGrant)`, which
  is unique per (workspace, provider, account_subject) and refuses an account
  another member connected.
- **The first-run UI** (#451) sends "Sign in" to `/bff/login`, then asks for
  a workspace (where the signup policy applies).

### The bar (from ADR 0004) and where each item lands

| Must-have | Here |
| --- | --- |
| Auth code + PKCE, S256 only; single-use codes | `/oauth2/authorize` refuses `plain` and a missing challenge; codes hashed, 60 s, single use; a replayed code revokes what it issued |
| client_credentials | workspace clients (confidential), `client_secret_basic` and `client_secret_post` |
| Refresh rotation (+ reuse handling, which Rauthy lacked) | a new refresh token on every use; a reused one revokes its whole family |
| JWT access tokens + JWKS, key rotation | RFC 9068 `at+jwt`, EdDSA (Ed25519); keys rotate every 30 days; `/oauth2/jwks` |
| Custom scopes | the catalogue `Module.scopes` already builds; nothing to mirror any more |
| Audience / RFC 8707 | `resource` is checked against the instance's resource list; `aud` = the API's canonical URI |
| API-driven client registration, per workspace | `identity.register_client` under `/api/v1/w/{workspace_id}/…`; DCR and Client ID Metadata Documents for MCP |
| Per-client claims (`workspace_id`) | workspace clients and workspace-bound user tokens carry `workspace_id` |
| Themable login + passkeys | our own screens; WebAuthn relying party in `identity` |
| Social upstream | Google, Microsoft, Zoho, **several per account** |
| Postgres storage | schema `identity` |
| Zero-manual-step upgrades | migrations + keys generated at start; nothing to provision |

## Owner decisions (2026-10-13)

The design was approved with these answers, recorded here and in the spec
deltas:

1. **Microsoft accepts personal, work and school accounts** (`common`). The
   issuer is checked against each token's own `tid`.
2. **Disconnecting a mailbox keeps its sign-in.** Mailbox connections and
   ways in are independent lifecycles.
3. **Passkey-only accounts are allowed.**
4. **Gmail stays in the sign-up consent.** The unverified-app warning is
   accepted until Google's verification (campbooks-cloud#53) passes. There
   is no fallback switch to a second consent.

## This change: slice S1a

Built here: the issuer (keys, discovery, authorize, token, revoke,
end_session), the first-party clients, Google sign-in with the new-account
step, held mailbox grants and their hand-off through `GrantIntake`, and the
BFF signing in against the in-process issuer. Rauthy keeps serving installs
whose `CAMPBOOKS_AUTH__ISSUER` names it; the issuer runs only when that
setting is unset (or names this instance) and the secret key is set.

Decided while building it, against the first draft below:

- **First-party clients are defined in code**, not stored: their redirect
  URIs follow `CAMPBOOKS_PUBLIC_URL` and the web client's secret is derived
  from the instance secret (`kernel::crypto::derive_secret`), which the BFF
  derives too. There is no `clients` table until workspace clients (S4).
- **Keys are scheduled on demand, not by a job.** The issuer checks the
  schedule (under an advisory lock) whenever it signs or publishes and its
  view is a minute old. An idle instance issues no tokens and needs no
  rotation.
- **Held grants and pending sign-ups are deleted, not revoked at Google,**
  when they expire: Google revokes every token of an account's grant to the
  same app at once, which would cut a mailbox connected through that app.
  Their sealed tokens are destroyed.
- **The BFF reaches the issuer through its router** (an in-process transport
  in the BFF's OIDC client), not over the network. The protocol is the same.
- **Access-token revocation is not tracked**: they live ten minutes;
  `/oauth2/revoke` revokes refresh tokens.
- **Rate limits are deferred** to a follow-up before #57: behind a reverse
  proxy they need a trusted-proxy setting to tell clients apart, or they
  would throttle everyone at once.

## Data ownership

Schema **`identity`**, owned by the new module and migrated by
`<ts>_identity__<desc>.sql`. Accounts, credentials, sessions and the
authorization server's state belong to people and clients, not to a
workspace, so those tables carry **no** row-level security. Like
`workspaces.users` and `bff.sessions`, only `identity` reads them. The one
workspace-owned table, `clients` (slice S4), will be under forced RLS
(below). The block below is the full model across slices; slice S1a creates
`accounts`, `provider_identities`, `sessions`, `authorize_requests`,
`upstream_flows`, `pending_sign_ups` (a provider identity waiting for the
person to confirm a new account), `held_grants`, `refresh_families`,
`refresh_tokens`, `authorization_codes`, `signing_keys` and `audit_events`.

```text
accounts            id uuid (v7) = token `sub` · display_name · primary_email ·
                    primary_email_verified · created_at · disabled_at
provider_identities id · account_id → accounts · provider (google|microsoft|zoho) ·
                    issuer · subject · email_at_link · email_verified_at_link ·
                    linked_via (sign_up|link|connect) · linked_at · last_used_at
                    UNIQUE (provider, issuer, subject)
passkeys            id · account_id · credential_id bytea UNIQUE · public_key (COSE) ·
                    alg · sign_count · aaguid · transports text[] · backup_eligible ·
                    backup_state · name · created_at · last_used_at
sessions            the issuer's browser sessions (SSO): id · token_hash UNIQUE ·
                    account_id · method (provider|passkey) · credential id · auth_time ·
                    user_agent · created_at · idle_expires_at · expires_at · ended_at
upstream_flows      one provider round trip: state_hash PK · browser_hash · provider ·
                    purpose (sign_in|link|connect|reconnect) · account_id? ·
                    workspace_id? · connection_id? · authorize_request_id? ·
                    nonce · code_verifier (sealed) · return_to · expires_at
authorize_requests  a parsed /oauth2/authorize waiting for sign-in or consent:
                    id · client_id · redirect_uri · scope · state · nonce ·
                    code_challenge · resource · prompt · max_age · expires_at
webauthn_challenges challenge_hash PK · purpose · account_id? · expires_at
held_grants         a mailbox grant waiting for a workspace: id · account_id ·
                    provider · issuer · subject · address · display_name · scopes ·
                    tokens (sealed) · expires_at
clients             client_id PK · kind (first_party|workspace|dynamic|metadata_document) ·
                    workspace_id? · name · logo_uri · redirect_uris text[] ·
                    grant_types text[] · auth_method (none|client_secret_basic|
                    client_secret_post) · secret_hash · allowed_scopes text[] ·
                    created_by · created_at · last_used_at · disabled_at
consents            account_id · client_id · workspace_id · scopes · granted_at ·
                    revoked_at
authorization_codes code_hash PK · client_id · account_id · session_id · redirect_uri ·
                    scope · resource · workspace_id? · code_challenge · nonce ·
                    auth_time · expires_at · redeemed_at
refresh_families    id · client_id · account_id? · scope · resource · workspace_id? ·
                    session_id? · created_at · revoked_at · revoke_reason
refresh_tokens      token_hash PK · family_id · issued_at · expires_at · rotated_at
signing_keys        kid PK · alg (EdDSA) · public_jwk jsonb · private_key (sealed) ·
                    created_at · publish_at · activate_at · retire_at · delete_after
audit_events        id · account_id? · client_id? · workspace_id? · kind · at ·
                    metadata jsonb (ids, provider, method; never emails or tokens)
```

- **Secrets at rest.**
  - Every token, code and challenge we issue is a random 256-bit value and
    is stored only as its SHA-256.
  - Client secrets are 256-bit random values stored as SHA-256. Their
    entropy makes a slow hash unnecessary.
  - What we must read back (the PKCE verifier sent upstream, held provider
    tokens, private signing keys) is sealed with `kernel::crypto::Cipher`
    (AES-256-GCM, HKDF from `CAMPBOOKS_SECRET_KEY`, purpose-labelled, bound
    to table, column and row; moved to the kernel by #450).
- **RLS on `clients`.** The table has `workspace_id` (NULL for first-party,
  dynamic and metadata-document clients), so it gets the standard forced
  `workspace_isolation` policy for the management API. It also gets one
  `FOR SELECT` policy admitting rows when
  `campbooks.identity_role = 'authorization_server'`. Only the
  authorize/token code path sets that, `LOCAL` to its transaction, because
  it must find a client by id before it knows the workspace. The server's
  RLS test keeps passing (the policy is forced).
- **`workspaces`** needs no schema change. `users.idp_subject` now holds the
  account id. The `email` and `email_verified` claims come from the account's
  primary email.
- **Pruning.** `identity.prune` (schedule, every 15 min) deletes expired
  flows, requests, pending sign-ups, held grants, codes, ended sessions and
  refresh families a day past their end. Keys past `delete_after` are
  deleted by the key schedule itself.

## Events

Published (instance-scoped, `WORKSPACE_SCOPED = false`, except where a
workspace is named), each declared with `Background::publishes`. Slice S1a
publishes the first three; the rest arrive with their slices:

- `identity.account_created`
- `identity.signed_in`
- `identity.refresh_reuse_detected`
- `identity.way_in_added` (provider or passkey)
- `identity.way_in_removed`
- `identity.client_registered` (workspace)
- `identity.client_removed` (workspace)
- `identity.consent_granted` (workspace)
- `identity.consent_revoked` (workspace)

Payloads hold ids, the provider name and the method, never addresses or
tokens.

Consumed (from slice S4, when consents and workspace clients exist):
`workspaces.member_left` and `workspaces.workspace_deleting`, so that
consents and workspace-bound refresh families for that member or workspace
are revoked, and the workspace's clients are removed. Slice S1a consumes
nothing.

## Endpoints and scopes

Protocol endpoints are standard OAuth/OIDC, documented in the OpenAPI
document under the `identity` tag with `operation_id` `identity.<verb>`,
`security(())` (client authentication is part of the protocol) and an
`x-campbooks` block opted out of automation and the assistant. `cargo xtask
arch` learns that `/oauth2/*`, `/.well-known/*` and `/auth/*` are the
identity module's routes and may sit outside `/api/v1`.

| Path | Purpose | Security |
| --- | --- | --- |
| `GET /.well-known/openid-configuration`, `/.well-known/oauth-authorization-server` | issuer metadata (incl. `code_challenge_methods_supported: [S256]`, `client_id_metadata_document_supported`, `registration_endpoint`) | public |
| `GET /.well-known/oauth-protected-resource` | RFC 9728, for the API resource | public |
| `GET /oauth2/authorize` | start or continue an authorization; renders nothing: 303 to the SPA's sign-in or consent route, or back to the client | public |
| `POST /oauth2/token` | authorization_code, refresh_token, client_credentials | client auth |
| `POST /oauth2/revoke` | RFC 7009 | client auth |
| `GET /oauth2/jwks` | public keys (published + active + retiring) | public |
| `GET /oauth2/userinfo` | OIDC userinfo | bearer, `openid` |
| `GET|POST /oauth2/end_session` | RP-initiated logout | public, `id_token_hint` or first-party session |
| `POST /oauth2/register` | RFC 7591 DCR (public clients only) | public, rate-limited |
| `GET /auth/start/{provider}` | start a provider round trip (`purpose`, `return_to`, `request`) | public + login cookie |
| `GET /auth/callback/{provider}` | finish it | public, state bound to the browser |
| `GET /auth/request/{id}` | what the sign-in / consent screen shows (client name, logo, scopes, redirect host, workspaces) | issuer session cookie |
| `POST /auth/new-account`, `/auth/consent`, `/auth/passkey/*` | screen actions | issuer cookie + CSRF |

API operations (resource server, `/api/v1/…`), new scopes in
`identity::scopes`:

| Operation | Scope |
| --- | --- |
| `identity.get_account`, `identity.list_ways_in`, `identity.list_sessions`, `identity.list_consents` | `identity:read` |
| `identity.update_account`, `identity.remove_way_in`, `identity.start_link`, `identity.register_passkey`, `identity.end_session`, `identity.revoke_consent`, `identity.adopt_held_grant`, `identity.discard_held_grant` | `identity:write` |
| `identity.list_clients`, `identity.register_client`, `identity.rotate_client_secret`, `identity.remove_client` under `/api/v1/w/{workspace_id}/identity/clients` | `identity:clients`, role owner/admin |

## Flows

### Sign-up and sign-in with a provider (web)

1. The SPA's signed-out landing (#451) offers each configured provider plus
   "Use a passkey". It links to `/bff/login`.
2. The BFF sends the browser to `/oauth2/authorize` (client `campbooks-web`,
   PKCE). There is no issuer session, so the issuer stores the request and
   303s to `/sign-in?request=<id>`.
3. The person picks Google, which goes to
   `/auth/start/google?purpose=sign_in&request=<id>`. A flow is stored
   (state, nonce, sealed PKCE verifier, browser-bound `SameSite=Lax` cookie).
   The browser is sent to Google with `openid email profile` + the scopes
   `GrantIntake::scopes(google)` returns (`gmail.modify` today), plus
   `access_type=offline`, `include_granted_scopes=true` and
   `prompt=select_account`.
4. `/auth/callback/google` checks state, browser, `iss` and the provider
   error. It exchanges the code with the verifier, then validates the ID
   token: signature against Google's JWKS through the kernel's cache,
   issuer, audience, `azp`, expiry and nonce.
   - **Known `(provider, issuer, sub)`:** the issuer session starts for that
     account. If Google returned a refresh token (a fresh consent), it is
     **not** used for existing connections: a sign-in never changes a
     workspace's mailbox behind the member's back. It is dropped, not
     stored (and not revoked: that would also cut the mailbox connection).
   - **Unknown:** nothing is created yet. The flow becomes a
     pending sign-up (sealed tokens included) and the browser goes to
     `/sign-in/new?request=<id>`: "No Campbooks account uses
     ana@example.com yet. Create one, or go back and sign in another way to
     add it later." The page never says whether any other account has that
     address. **Create** makes the account, the provider identity
     (`linked_via = sign_up`) and the held grant (when a refresh token and
     mail scopes were granted), in one transaction.
   - **No refresh token on a new account** (the person had consented to this
     Google app before, e.g. an earlier deleted account): one more round
     trip with `prompt=consent` and `login_hint`, transparently.
5. The issuer completes the stored authorization request: code → BFF
   callback → BFF session, as today.
6. The first-run UI asks for a workspace. Its last step reads "Your mail
   (ana@example.com) will be connected here" (on by default, can be turned
   off). After `workspaces.create`, the UI calls
   `identity.adopt_held_grant {workspace_id}`: `identity` resolves nothing
   itself. It calls `GrantIntake::adopt(ProviderGrant { workspace, subject:
   account id, provider, account_subject, address, display_name,
   capabilities, tokens })` and deletes the held grant on success.
   `connectors` maps the subject to the member through the `Membership`
   port, as it does for its own operations.

If Google's granular consent unchecks the Gmail scope, sign-in still works.
No grant is held, and Home offers "Connect your mail".

### Adding a way in, connecting another mailbox, reconnecting

Signed in, the SPA calls `identity.start_link {provider, workspace_id?,
connection_id?}`. It returns the URL of `/auth/start/{provider}` with a flow
bound to the account, with `prompt=consent` so a refresh token always comes
back.

- `purpose = link` (from Settings → Ways to sign in): identity scopes only,
  unless a workspace is given.
- `purpose = connect` (from a workspace's "Connect a mailbox", which
  replaces #450's `start_connection`): identity + mail scopes.
- `purpose = reconnect` (a connection in `needs_reauth`): the same, with
  `extending = connection_id`.

On callback the provider identity is linked to the account. If the account
already holds it, the link is refreshed. If **another account** holds it,
the flow is refused with `way_in_belongs_to_another_account`: "This Google
account is already a way into another Campbooks account. Sign in with it to
use that account." Nothing is merged or moved, and the tokens are revoked.
For `connect` and `reconnect` the grant is then adopted into the workspace
at once. `connectors`' own refusals (another member's connection, account
mismatch) come back as problems the SPA already handles. Every connected
OAuth mailbox is therefore a way in by construction.

### Removing a mailbox or a way in

Two separate lifecycles:

- Disconnecting a mailbox (a workspace connection) leaves the way in. One
  Google account can be connected in several workspaces, and a way in
  belongs to the person.
- The disconnect confirmation says "You can still sign in with this account;
  remove it under Ways to sign in."
- Removing a way in (`identity.remove_way_in`) never disconnects a mailbox,
  and is refused with `last_way_in` when it is the account's only provider
  identity or passkey. The account row is locked in that transaction.
- Removing a way in also needs a recent sign-in (step-up, below).

### Passkeys

Relying-party id = the host of `CAMPBOOKS_PUBLIC_URL`; origin = that origin.
Native apps use the system browser, so the same origin.

- **Register** (signed in, step-up): `navigator.credentials.create` with
  `userVerification: required`, `residentKey: required`, attestation
  `none`, algorithms EdDSA (-8), ES256 (-7), RS256 (-257).
- **Sign in**: a discoverable credential (`allowCredentials` empty)
  through conditional UI on the sign-in page, plus a "Use a passkey" button.
- **Sign up with a passkey alone**: display name, then create. Allowed
  because self-hosters may configure no provider app and IMAP users have no
  OAuth proof; the signup policy still applies when a workspace is created.
- Verification (WebAuthn L3 §7.1/§7.2):
  - type, challenge (single use, 5 min), origin and `topOrigin` absent;
  - `rpIdHash`, the UP and UV flags, BE/BS consistency;
  - the COSE key parsed with `coset` and the signature checked with
    `aws-lc-rs`;
  - a sign count that goes backwards (when non-zero) is recorded as an
    audit event, not a refusal, because synced passkeys report 0.

### Native apps (Tauri, #17)

`campbooks-native` is a public client created by migration with redirect
URIs:

- `http://127.0.0.1/callback`, matched on any port (RFC 8252 §7.3);
- `com.notacamp.campbooks:/oauth/callback`, a private-use scheme (RFC 8252
  §7.1) for mobile.

It uses auth code + PKCE in the system browser. Refresh tokens live in the
OS keychain on the Rust side, rotate on every use and are revoked as a
family on reuse. No consent screen (first-party), no `workspace_id` claim.

### Third-party clients and MCP agents

- **Workspace clients** (Settings → API access, owners and admins):
  - confidential clients get client_credentials and a `workspace_id` claim;
    the secret is shown once and can be rotated;
  - public clients use auth code + PKCE with exact redirect URIs.
- **DCR** (`POST /oauth2/register`):
  - public clients only (`token_endpoint_auth_method = none`), auth code +
    PKCE, `https` or loopback redirect URIs;
  - rate-limited per IP; no registration access token (RFC 7592 not
    offered);
  - a client unused for 30 days is deleted;
  - shown on consent as "unverified" with the redirect host.
- **Client ID Metadata Documents** (MCP 2025-11-25 recommends them over
  DCR): an `https` `client_id` with a path is fetched through an SSRF guard
  (public addresses only, no redirects, 5 s, 64 KiB, cached by
  `Cache-Control` up to 24 h). Its `client_id` must equal the URL, and
  redirect URIs come only from the document. Loopback-only clients get the
  extra warning the MCP spec asks for.
- **Consent** (every non-first-party client): the client's name, its logo
  (only from a workspace client), the redirect host in full, the scopes in
  plain words (`ScopeName::DESCRIPTION`, translated), and **which workspace**
  it may use. The resulting tokens carry that `workspace_id`.
  - `kernel::tenancy::Workspace<S>` already requires a client token's
    `workspace_id` to name the path's workspace; it will require the same of
    any token that carries the claim. Membership is still checked.
  - Consent is remembered per (account, client, workspace, scopes) and is
    revocable from Settings.
- `resource` (RFC 8707): accepted values are the API's canonical URI
  (`<app-origin>/api`, the default `CAMPBOOKS_AUTH__AUDIENCE`) and,
  later, the MCP endpoint. Anything else is `invalid_target`.

### Tokens

- **Access token**: JWT, header `typ: at+jwt`, `alg: EdDSA`, `kid`.
  - Claims: `iss`, `sub` (account id; client id for client_credentials),
    `aud` (the resource), `client_id`/`azp`, `scope`, `iat`, `nbf`, `exp`,
    `jti`, `auth_time`, and `workspace_id` when bound.
  - `email`, `email_verified` and `name` only for first-party clients or
    when `email`/`profile` were consented.
  - Lifetime 10 minutes.
- **ID token** (first-party clients and `openid` requests): EdDSA, 10
  minutes, `nonce`, `sid`, `auth_time`, `amr` (`fed` for a provider,
  `hwk`/`swk` + `user` for a passkey).
- **Refresh token**: opaque.
  - BFF: 14 days idle, 30 days absolute.
  - Native: 30 days idle, 90 days absolute.
  - Third-party: 30 days idle, until consent is revoked.
  - Reuse of a rotated token revokes the family and emits
    `identity.refresh_reuse_detected`. There is no grace window: the BFF
    and the native shell already refresh single-flight.
- **Authorization code**: 60 s, single use. A second redemption revokes the
  family it created (RFC 6749 §4.1.2).

### Sessions and step-up

The issuer's browser session (`__Host-campbooks_auth`, `HttpOnly`, `Secure`,
`SameSite=Lax`) gives single sign-on across the BFF, the native app's system
browser and third-party authorizations: 14 days idle, 30 days absolute. It
is `Lax`, not `Strict`, because a third-party authorization arrives as a
cross-site navigation.

The BFF session stays as it is. `/bff/logout` revokes the BFF's refresh
family and ends the issuer session in-process. There is no end-session
confirmation page, which resolves the Rauthy logout question in
`decisions-needed.md`.

**Step-up**: adding or removing a way in, registering a passkey, and
creating a client secret need `auth_time` within 10 minutes; otherwise the
SPA re-authenticates (`max_age=600`) with any of the account's ways in.

**MFA stance**: Campbooks has no password to protect. Providers enforce
their own MFA, and passkeys require user verification. `amr` is recorded,
so a future policy ("passkey required for admins") is a check, not a
redesign.

## Decisions and alternatives considered

The old Rails app on `main` (`Identity` + `Auth::OauthSignIn`, where every
owned mailbox signs you in) was read as prior art.

- **Copied on the merits:** the product rule itself (every owned OAuth
  mailbox is a way in), and one consent for sign-in plus mail.
- **Not copied:**
  - its linking by verified email: we link only by a completed round trip;
  - its passwords;
  - Doorkeeper-style client_credentials-only issuance;
  - its per-provider controllers: we use one front door with purposes.

### 1. Who owns accounts and credentials

- **A. New module `identity` (chosen).** It owns its schema and depends on
  the kernel only; it reaches `connectors` through a kernel port and
  `workspaces` only through events. Accounts are not tenancy, and
  credentials and token state are a large, security-critical surface that
  deserves its own crate, tests and reviewers.
- **B. Inside `workspaces`.** `users` already exists, so there would be no
  subject indirection. But it mixes authentication with tenancy, makes the
  module that every `Workspace<S>` call depends on carry WebAuthn and token
  issuance, and the issuer's state isn't workspace-owned anyway.
- **C. In `campbooks-server`, like the BFF.** It could call `connectors`
  directly with no port. But the composition root would hold domain rules
  and a schema, with tests only at server level, and arch's "modules depend
  on kernel only" would no longer describe where the most sensitive code
  lives.
- **D. In the kernel.** The kernel knows no domain and owns no tables beyond
  infrastructure. Rejected outright.

### 2. Build the authorization server on a crate, or hand-roll it

Candidates, checked 2026-10-13 against crates.io, upstream repositories and
`deny.toml` (licences allowed: MIT, Apache-2.0, BSD-3-Clause, ISC,
Unicode-3.0, Zlib, CDLA-Permissive-2.0; OpenSSL banned):

| Crate | Version (date) | Licence | Verdict |
| --- | --- | --- | --- |
| `oxide-auth` (+ `-axum`) | 0.6.1 (2024-06-02); axum adapter 0.6.0 (2025-01-09) | MIT/Apache-2.0 | Stale (no release in 28 months), OAuth 2 only: no OIDC, DCR or refresh-reuse detection; `rand` 0.8, `rust-argon2`. We'd write most of the profile anyway. **No.** |
| `openidconnect` | 4.0.1 (2025-07-06) | MIT | Client side only. Pulls `oauth2` 5 (reqwest 0.12 vs our 0.13), `rand` 0.8, `base64` 0.21 and RustCrypto `rsa` 0.9 (RUSTSEC-2023-0071, no fixed release, so a `deny.toml` ignore would be needed). Our kernel already validates ID tokens and caches JWKS. **No.** |
| `jsonwebtoken` | 11.1.0 (2026-09-16), **already in the graph** | MIT | Signing and verifying EdDSA with the `aws_lc_rs` backend. **Yes.** |
| `aws-lc-rs` | 1.18.1, **already in the graph** | ISC/Apache-2.0 | Ed25519 key generation (TestIdp already does it), ES256/RS256/EdDSA verification for passkeys, CSPRNG. **Yes.** |
| `ed25519-dalek` | 3.0.0 (2026-07-06) | BSD-3-Clause | Redundant with `aws-lc-rs`. **No.** |
| `josekit` | 0.10.3 (2025-05-20) | MIT/Apache-2.0 | Depends on `openssl`: banned. **No.** |
| `webauthn-rs` | 0.5.5 (2026-04-30) | **MPL-2.0** | The best-reviewed WebAuthn RP in Rust, but `webauthn-rs-core` depends on `openssl` unconditionally (banned) and MPL-2.0 is not on our licence list. **No.** |
| `webauthn_rp` | 0.3.0 (2025-04-03) | MIT/Apache-2.0 | Small user base (~33k downloads), pulls RustCrypto `rsa` 0.9 (advisory) and `ed25519-dalek` 2. **No.** |
| `passkey` (1Password) | 0.6.0 (2026-10-01) | MIT/Apache-2.0 | Authenticator and client side, not a relying party; too new for the two-week rule anyway. **No.** |
| `coset` | 0.4.2 (2026-03-02) | Apache-2.0 | COSE_Key parsing for passkeys (Google). Small, maintained. **Yes (S3).** |
| `ciborium` | 0.2.2 (2024-01-24) | Apache-2.0 | CBOR, `coset`'s own dependency, for attestation objects. **Yes (S3)**, through `coset`. |

- **A. Hand-rolled on `jsonwebtoken` + `aws-lc-rs` (chosen).**
  - Narrow profile: authorization_code + PKCE S256, refresh_token,
    client_credentials. No implicit, password, device or hybrid flows.
  - Every MUST of OAuth 2.1, RFC 7636, 7009, 8414, 8707, 9068, 9207, 9728,
    7591 and OIDC Core/Discovery for that profile becomes a named test (the
    `tasks.md` checklist).
  - The OpenID Foundation conformance suite (hosted; no local Docker) runs
    against staging before #57.
  - Two new crates in total, both only for passkeys.
- **B. `oxide-auth`.** See the table. It would save the code-grant state
  machine but not the hard parts (OIDC, rotation and reuse, DCR, consent),
  and ties us to a stalled crate.
- **C. Ory Hydra headless + our login/consent app.** It is genuinely
  attractive: certified issuance, DCR, rotation, and login/consent
  delegated to us, so the identity rules stay ours. But it is a second
  service in Go with its own schema and a `hydra migrate` step on every
  upgrade, and client registration and consent become admin-API calls
  between two processes. Self-hosters would gain one service, not lose
  three. **This is the fallback** if A fails conformance or review.

### 3. Upstream providers

| | Google | Microsoft | Zoho |
| --- | --- | --- | --- |
| Endpoints | `https://accounts.google.com` (discovery) | `https://login.microsoftonline.com/common/v2.0` | `https://accounts.zoho.<dc>` per data centre |
| Identity key | `(google, iss, sub)` | `(microsoft, iss with the token's tid, sub)` | `(zoho, accounts server, sub)` |
| Issuer check | exact | `iss` must equal `https://login.microsoftonline.com/{tid}/v2.0` for the token's own `tid`; keys from the common JWKS | `iss` must equal the accounts server the code was exchanged at |
| Email verified | `email_verified` | only with the optional `xms_edov` claim = true; otherwise unverified | `email_verified` |
| Offline access | `access_type=offline`; a refresh token only on a fresh consent, hence the `prompt=consent` retry | `offline_access` scope | `access_type=offline`; `prompt=consent` on link/connect |
| Mail scopes | from `GrantIntake::scopes` (`gmail.modify`, restricted: #53) | Graph `Mail.ReadWrite`, `Mail.Send`, `User.Read` once the Graph adapter exists | Zoho Mail scopes once #74 ships |

- **Zoho data centres.** The callback carries `accounts-server`; it must be
  one of a fixed list of Zoho accounts servers (`.com`, `.eu`, `.in`,
  `.com.au`, `.jp`, `.ca`, `.sa`, `.uk`). Anything else is refused before
  any request, which closes the SSRF and token-exfiltration route. Users of
  every data centre can sign in, unlike with Rauthy.
- **Microsoft** uses the `common` endpoint: work, school and personal
  accounts (owner decision, 2026-10-13). The audience of young professionals and
  small businesses includes outlook.com users.
- **Providers whose adapter isn't built yet** (Microsoft, Zoho today):
  `GrantIntake::scopes` returns no mail scopes, so sign-in asks for identity
  only and nothing is held. When the adapter lands, "Connect your mail" uses
  the `connect` purpose.
- **Provider configuration** is shared with `connectors`:
  `CAMPBOOKS_GOOGLE__CLIENT_ID` / `__CLIENT_SECRET_FILE` (already defined by
  #450), `CAMPBOOKS_MICROSOFT__*`, `CAMPBOOKS_ZOHO__*`. Each module reads
  the same settings; `deploy/README.md` documents them once. A provider with
  no client id is hidden on the sign-in page.

Alternatives for linking:

- **(a) Proven ownership only (chosen).** Linking happens only through a
  round trip completed while signed in, or the one that created the
  account. It is safe against unverified-email takeover.
- **(b) Auto-link on verified email (Google and Zoho only).** Fewer
  duplicate accounts, but a provider's "verified" is a claim about a
  mailbox, not about the person holding our account. Recycled addresses and
  domain takeovers turn it into an account-takeover path. Rejected; the
  owner forbade it.
- **(c) Suggest a link when the email matches** ("an account with this
  address exists, sign in to link"). Better UX, but it reveals which
  addresses have accounts, and Microsoft's emails are unverified. Rejected;
  the new-account page says the same thing whatever the address.

### 4. How the web BFF signs in

- **A. The BFF stays an OIDC client of the in-process issuer (chosen).**
  - The code from #16 is kept, with its tests: PKCE, nonce, ID-token check,
    refresh single-flight, and the browser never sees a token.
  - Only its configuration changes: the issuer is the server's own public
    URL, and the client secret is derived (HKDF of `CAMPBOOKS_SECRET_KEY`,
    label `identity.client.campbooks-web`), so nothing is provisioned.
  - Its token and end-session calls run through the in-process router, as
    `/bff/api/*` does, so there are no hairpin requests through the proxy.
  - One token path for every client, and the API validates the BFF's
    tokens like anyone's.
- **B. The BFF talks to providers and mints its session directly; the API
  trusts an in-process principal.** One fewer hop and no tokens stored. But
  the BFF becomes a second authentication system: native, third-party and
  MCP still need the issuer, passkeys would be implemented twice, and the
  in-process principal is a path the resource server doesn't see.
- **C. Merge the BFF session and the issuer session into one cookie.** Fewer
  rows, but SSO for native and third-party sign-in would then depend on the
  BFF's `SameSite=Strict` cookie, and logging out of the web app would end
  every client's sign-in. Rejected.

### 5. Signing keys

- **Algorithm.**
  - **Ed25519 (EdDSA) only (chosen).** Small keys and signatures,
    deterministic, already accepted by the kernel verifier, generated by
    `aws-lc-rs`.
  - RS256 is what OIDC requires an OP to support for ID tokens to arbitrary
    relying parties, and "Sign in with Campbooks" is a non-goal; our ID
    tokens go only to our own clients. If that changes, an RS256 key is
    added beside it (the JWKS is a set).
  - Alternatives: RS256 everywhere (universal, but 2–3 KB keys, slow key
    generation, larger tokens); ES256 (universal-ish, but non-deterministic
    signatures and no gain over EdDSA for our verifier).
- **Lifecycle.** States are `published` → `active` → `retiring` →
  `deleted`.
  - The schedule is kept on demand: whenever the issuer signs or publishes
    and its view is a minute old, it checks under an advisory lock. It keeps
    exactly one active key, creates the next one 7 days before rotation
    (longer than the verifier's 24 h maximum JWKS cache), and activates it
    at 30 days. (The first draft had an hourly job; on demand needs no
    worker and does nothing for an idle instance.)
  - It keeps a retired key in the JWKS for 2 days (longer than any access or
    ID token, which live 10 minutes).
  - At first start, if no active key exists, one is created inside a
    transaction under an advisory lock, so many processes starting at once
    make one key.
- **Storage.** The private key (PKCS#8) is sealed with the module's
  `Cipher` (purpose `campbooks identity sealing v1`), bound to its `kid`. Self-hosters already have
  `CAMPBOOKS_SECRET_KEY` (generated by `server-init`, ADR 0006), so there
  is nothing new to set.
  - Rotating `CAMPBOOKS_SECRET_KEY` makes the sealed keys unreadable. The
    server then generates a new key set and every session ends; this is the
    existing BFF behaviour, now documented for both.
  - Alternatives: a key file in the `server-secrets` volume (one more file
    to back up, no rotation); a KMS (not self-hostable).
- **How the verifier finds the keys.** `kernel::auth::TokenVerifier` gets a
  `KeySource`: either the existing remote JWKS fetch, or a local source
  implemented by `identity` that returns the same JWKS document from the
  database. Parsing, the `kid`-miss refresh and the 30 s refresh floor are
  shared, so verification is byte-for-byte the code path third parties
  would use against `/oauth2/jwks`.
  - With `CAMPBOOKS_AUTH__ISSUER` unset, the issuer defaults to
    `CAMPBOOKS_PUBLIC_URL` and the source is local.
  - `TestIdp` keeps the remote source, so module tests are unchanged.

### 6. Holding and handing over the mailbox grant

- **A. `identity` holds it, sealed; the UI adopts it explicitly through
  `GrantIntake` (chosen).**
  - The first-run UI shows the address and a switch, then calls
    `identity.adopt_held_grant` after `workspaces.create`. If that call
    fails or the person joins a workspace by invitation instead, Home offers
    "Connect ana@example.com" with one click and no second consent.
  - 7-day expiry; an expired grant is deleted (not revoked at Google, which
    would also cut a mailbox connected through the same app).
  - Nobody's personal mailbox lands in a team workspace they were invited
    to without their say.
- **B. Automatic adoption on `workspaces.workspace_created`.** No extra
  call. But the event names the workspaces `UserId`, not the account, so
  `identity` would need a mapping it doesn't own. It also gives the person
  no say, and does nothing for the invitation path.
- **C. `connectors` holds it as a workspace-less connection.** It would
  keep all provider tokens in one module. But it breaks `connectors`'
  workspace-owned model (and its forced RLS), and a connection that belongs
  to nobody's workspace leaks into every query.

**Direction of the port.** `GrantIntake` is a kernel trait implemented by
`connectors` and registered in `AppState`, like `Membership`. It has two
methods:

- `scopes(provider) -> Vec<String>`: what mailbox capabilities this
  instance can use;
- `adopt(ProviderGrant) -> Result<ConnectionRef>`.

Tokens are `SecretString` and never enter an event or a job payload. The
alternative (`identity` publishes an event and `connectors` pulls tokens
through a port `identity` implements) was rejected: two hops for the same
synchronous need, and `connectors` would have to decide which workspace,
which only the person can.

### 7. One front door for every provider consent

- **A. `identity` runs every provider round trip (chosen).** That covers
  sign-in, link, connect and reconnect.
  - One redirect URI per provider, one implementation of state, nonce,
    PKCE and ID-token validation.
  - Every connected mailbox is a proven way in by construction.
  - `connectors` keeps the tokens' lifecycle (refresh, revoke, health) and
    its `create_or_refresh_connection` seam.
  - #450's `start_connection`/`complete_connection` become delegates (or
    are removed) in S2.
- **B. `connectors` keeps its own consent and publishes the account it
  connected; `identity` links on the event.** It leaves #450 untouched. But
  there would be two implementations of the upstream flow and two redirect
  URIs per provider. The event would have to carry the account subject and
  address (personal data in the retained log), and linking becomes eventual,
  so "connect, then sign out, then sign in with it" can race.

## Screens (clients/app, module `identity`)

All screens are built from `lib/ui` with tokens only, are keyboard-first
with visible hints, and come in en, pt-PT, es-ES and fr.

- **Sign-in** (`/sign-in`):
  - one button per configured provider, `1`/`2`/`3`, each with the
    provider's mark and its name;
  - "Use a passkey" (`P`), with conditional-UI autofill;
  - the client's name when a third party asked;
  - errors explained in place (`cancelled_at_provider`,
    `way_in_belongs_to_another_account`, `provider_unavailable`).
- **New account** (`/sign-in/new`): the address, then Create (`Enter`) or
  "Sign in another way" (`Esc`).
- **Consent** (`/consent`):
  - the client and its verification state, the redirect host, scopes in
    plain words, and a workspace picker (`↑↓`);
  - Allow (`⌘Enter`) or Deny (`Esc`);
  - extra warnings for loopback-only and unverified clients.
- **Settings → Ways to sign in:**
  - list, Add Google/Microsoft/Zoho, Add passkey, Rename and Remove (with
    `last_way_in` explained);
  - recent sign-ins and sessions (end one);
  - connected apps (revoke consent).
- **Settings → API access** (workspace): clients, secret shown once,
  rotate, remove.
- **First run** (#451): the "connect this mailbox here" step.

The `/auth/*` and `/oauth2/authorize` routes render no HTML themselves; they
redirect into these SPA routes. Every response carries
`frame-ancestors 'none'` and `Referrer-Policy: no-referrer`.

## Security posture

- **CSRF, state, nonce, PKCE everywhere.**
  - Upstream: state bound to a browser cookie, nonce, PKCE S256 and the
    `iss` check (RFC 9207 where the provider sends it), with distinct
    redirect URIs per provider against mix-up.
  - Downstream: PKCE S256 is required, `iss` is in every authorization
    response, and the nonce is echoed.
  - Screen actions (`POST /auth/*`) need `X-Campbooks-CSRF` and
    `Sec-Fetch-Site: same-origin` (the BFF's check, reused).
- **Open redirects.**
  - `redirect_uri` is matched exactly, except the loopback port for native
    clients.
  - An unknown client or unregistered redirect gets an error page, never a
    redirect.
  - `return_to` must be a same-origin path (the BFF's validator).
  - Zoho's `accounts-server` comes from the allowlist.
  - The Client ID Metadata Document fetch is SSRF-guarded.
- **Rate limits** (a follow-up before #57; see "This change") are
  Postgres-backed so every `api` process shares them,
  keyed by a keyed hash of the IP (from `CAMPBOOKS_TRUSTED_PROXIES`) and,
  where known, by the account. They cover `/auth/start`, the callbacks,
  passkey verification, `/oauth2/token` (and per client),
  `/oauth2/register` and account creation.
- **Refresh reuse** revokes the family, ends the issuer session it came from
  for public clients, and is audited.
- **Logging bans.** No token, code, verifier, state, nonce, client secret,
  cookie, assertion or email is ever in a log, span or metric. The
  telemetry privacy test gains the identity routes, and the request log
  already drops query strings.
- **Audit.** `identity.audit_events` plus the domain events above.
  Settings → Ways to sign in shows the account's own recent events.
- **Consent** for every non-first-party client, workspace-bound, revocable.
  DCR clients are labelled unverified.
- **Security review #57** must review this change (and the OIDF suite
  results) before production.

## Migration away from Rauthy

The deploy changes (S5):

- `deploy/compose.yaml` loses `idp`, `idp-init`, `idp-provision` and the
  `idp-data` volume.
- `server` loses `CAMPBOOKS_AUTH__ISSUER`/`JWKS_URL` (now defaults) and the
  `CAMPBOOKS_BFF__IDP_INTERNAL_URL`/`CLIENT_SECRET_FILE` settings.
- `server-init` keeps generating `secret-key` only.
- Removed outright: `deploy/idp/`, `xtask/tests/idp_provision.rs` and the
  Rauthy sections of `deploy/README.md`.
- Added to `deploy/README.md`: "Sign in with Google, Microsoft or Zoho",
  with one redirect URI per provider, and "Passkeys".
- `.env.example` and the CHANGELOG are updated, and the changelog says what
  a self-hoster sees: three fewer services, a one-time "sign in again", and
  provider settings renamed from `CAMPBOOKS_IDP_<P>_*` to `CAMPBOOKS_<P>__*`
  (the old names are still read for one release, with a warning, so
  upgrading needs no manual step).

Existing installs:

- Rauthy accounts are **not** migrated (no real users yet; the owner agreed
  that staging users can be dropped).
- People sign in again with a provider or a passkey, which creates fresh
  accounts. `workspaces.users` rows keyed by old Rauthy subjects become
  orphans; a one-off `workspaces` migration deletes users with no membership
  and leaves the rest for an operator to re-invite.
- The `idp-data` volume is left in place (compose does not delete volumes);
  the README says it can be removed.

Redirect URIs to register at each provider (one per provider, on the app
origin):

- Google: `<app-origin>/auth/callback/google`
- Microsoft: `<app-origin>/auth/callback/microsoft`
- Zoho: `<app-origin>/auth/callback/zoho`

The connectors callback page from #450 (`/connections/google/callback`) is
no longer needed once S2 lands.

The hosted staging environment's own changes (removing its IdP stack and
subdomain, registering the URIs above for its origin) live in the private
operations repository.

`openspec/changes/upstream-sign-in` is withdrawn; its useful findings
(provider endpoints, Microsoft's missing `email_verified`, Zoho data
centres) are carried here. The `decisions-needed.md` entries on the Rauthy
bump, the IdP origin, the web logout page and one provider per account are
removed as resolved.

## Slicing plan

Each slice is one PR with its own recording.

| Slice | Scope | Depends on | Recording |
| --- | --- | --- | --- |
| **S1a — Issuer core + Google + BFF** | `identity` module and schema; signing keys (generate, seal, rotate, JWKS); the kernel `KeySource`; discovery; authorize/token (code+PKCE, refresh with reuse detection) and revoke/end_session for the first-party clients; issuer sessions; the Google round trip (sign_in); new account; held grant + `adopt_held_grant`; the `GrantIntake` kernel port + the `connectors` implementation; BFF switched to the in-process issuer | #450 merged | vhs: server against a wiremock Google, curl through sign-up → token → API call → refresh reuse revoked |
| **S1b — Sign-in screens** | `/sign-in`, `/sign-in/new`, the first-run "connect here" step (on #451) | S1a's OpenAPI (can start on stubs in parallel with S1a) | 375 px + desktop: sign up with (fake) Google → first workspace → mailbox connected |
| **S2 — Ways in + Microsoft + Zoho** | link/connect/reconnect purposes; `remove_way_in` with `last_way_in`; step-up; Microsoft (`tid` issuer) and Zoho (data-centre allowlist); `connectors`' consent endpoints delegate to `identity`; Settings → Ways to sign in | S1 | UI: add Zoho as a way in, sign out, sign in with it; remove refused for the last way |
| **S3 — Passkeys** | WebAuthn RP (`coset`, `aws-lc-rs`): register, sign in (conditional UI), passkey-only sign-up | S1 | UI: add a passkey (virtual authenticator), sign in with it |
| **S4 — Native + third-party + MCP** | `campbooks-native` (loopback/private scheme); workspace clients API + Settings → API access; client_credentials; consent screen; workspace-bound tokens (`Workspace<S>` checks the claim on user tokens); DCR; Client ID Metadata Documents; RFC 9728 metadata + `WWW-Authenticate resource_metadata` | S1 (S4 may split into S4a clients/consent and S4b DCR/CIMD) | UI: register a client, run an MCP-style auth-code flow, consent, call the API; vhs for client_credentials |
| **S5 — Remove Rauthy** | compose, `deploy/idp`, xtask test, docs, settings rename with fallback, `decisions-needed.md`, CHANGELOG; the hosted staging cutover in the private repo | S1 | vhs: `docker compose config` service list before/after; sign-in on staging |

Order: S1a first; S1b in parallel with it (against the contract). Then S2,
S3, S4 and S5 can run **in parallel**: S2 touches `connectors` and the
account settings, S3 is `identity` + passkey screens, S4 touches the kernel
tenancy extractor + client screens, S5 is deploy and docs. After S4: the
OpenID conformance suite against staging, then security review #57 before
production.

## Risks and trade-offs

- **We own an authorization server.** The narrow profile, RFC-by-RFC tests,
  the OIDF suite and #57 mitigate it; Hydra remains the fallback.
- **Hand-rolled WebAuthn verification.** The scope is limited (attestation
  `none`, three algorithms, no metadata service), with test vectors from the
  WebAuthn spec and recorded browser assertions; the `webauthn-rs` route
  reopens if its OpenSSL dependency goes away.
- **Google sign-up shows the unverified-app screen until #53**, and Google
  apps left in "Testing" issue refresh tokens that expire after 7 days; the
  self-host docs must say to publish the app. The owner accepted the warning
  (2026-10-13): one consent, no second-consent fallback switch.
- **Accounts are created freely**, while the signup policy gates workspaces
  (as today). A stranger can create an account that can do nothing. It is
  rate-limited, and an account with no workspace and no sign-in for 30 days
  is deleted with its held grant.
- **Duplicate accounts.** Someone who signs up with Google and later tries
  Microsoft without linking gets the new-account page, not a silent second
  account; merging stays out of scope.
- **Passkey-only accounts have no verified email**, so they cannot accept an
  email invitation under `workspaces`' current rule. That is a follow-up for
  `workspaces` (accept by token holder plus confirmation), and does not block
  this change.
