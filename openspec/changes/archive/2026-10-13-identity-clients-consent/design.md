# Design

## Context

ADR 0014 and the S1a design
(`openspec/changes/archive/2026-10-13-campbooks-identity-s1a/design.md`)
already sketch slice S4:

- a `clients` table under forced row-level security;
- consent remembered per (account, client, workspace) and revocable;
- workspace-bound tokens checked by `Workspace<S>`;
- DCR for public clients, labelled unverified and pruned after 30 days;
- Client ID Metadata Documents behind an SSRF guard;
- `resource` checked against the instance's resources.

This document decides what those left open, records where it departs from
the sketch, and why:

- how a native client is made safe;
- when a consent may be skipped;
- the audience;
- which scopes a third party may hold;
- how the consent screen learns the person's workspaces;
- how the `resource_metadata` challenge is added.

What exists on `next`:

- The issuer:
  - first-party clients in code (`domain/clients.rs`);
  - `/oauth2/authorize` completes a stored request through
    `authorize::complete_stored`, the one place both the signed-in path and
    every sign-in end go through;
  - `/oauth2/token` (code, refresh) and `/oauth2/revoke`.
- `Principal.auth_time` and `identity::domain::step_up::require_recent` (S2).
- The kernel rate limiter: `kernel::rate_limit`, `ClientIp` behind
  `CAMPBOOKS_TRUSTED_PROXIES` (S2 merged it from this branch).

## Data ownership

Schema **`identity`**, migration `<ts>_identity__clients.sql`:

```text
clients             client_id text PK · kind (workspace|dynamic|metadata_document) ·
                    workspace_id uuid? (kind = workspace) · name · logo_uri? ·
                    redirect_uris text[] · grant_types text[] ·
                    auth_method (none|client_secret_basic) · secret_hash bytea? ·
                    allowed_scopes text[] · created_by uuid? (account) ·
                    created_at · secret_rotated_at? · last_used_at? ·
                    metadata_expires_at? (metadata documents: when to fetch again)
consents            id uuid PK · account_id → accounts · client_id · bound_workspace_id uuid? ·
                    bound_member uuid? (the workspaces UserId at grant time) ·
                    scopes text[] · remembered bool · granted_at · last_used_at? ·
                    revoked_at?   UNIQUE (account_id, client_id) WHERE revoked_at IS NULL
authorize_requests  + resource · bound_workspace_id? · consent_due (bool)
authorization_codes + resource · bound_workspace_id?
refresh_families    + resource · bound_workspace_id?
```

**Row-level security.**

- `clients` carries the forced `workspace_isolation` policy. Workspace clients
  are workspace-owned, and the management API reads them in
  `begin_for(workspace)`.
- `clients` also carries one `authorization_server` policy (all commands)
  for rows read or written by the protocol path. That path must find a
  client by id before it knows any workspace, and it inserts DCR and
  metadata-document rows, which belong to no workspace. Only identity's
  protocol code sets `campbooks.identity_role = 'authorization_server'`,
  `LOCAL` to its transaction (`domain::clients::begin_as_server`).
- The server's "every workspace-owned table has forced RLS" test passes.

**`bound_workspace_id`, not `workspace_id`, on consents, codes and families.**

- These rows belong to an account, not to a workspace: Settings lists
  every consent of a person across all their workspaces, and the token
  endpoint looks families up by token hash.
- The column records which workspace the token is *bound to*: a claim value,
  not ownership.
- Naming it `workspace_id` would put the token path under RLS for no
  isolation gain. Every query that reads these rows is already keyed by
  account or by a 256-bit token hash.

**Secrets.**

- A workspace client's secret is a 256-bit random value. Only its SHA-256 is
  stored; its entropy makes a slow hash pointless.
- It is shown once at creation and at each rotation.
- It is compared in constant time against the stored hash.

## Events

Published (with the workspace when there is one), ids and names only:

- `identity.client_registered` v1 `{ client, kind, workspace? }`.
  Workspace clients only. DCR and metadata clients are anonymous and many, so
  they are audited, not announced.
- `identity.client_removed` v1 `{ client, workspace }`
- `identity.consent_granted` v1 `{ account, client, workspace? }`
- `identity.consent_revoked` v1 `{ account, client, workspace? }`

Consumed:

- `workspaces.member_left` → `identity.revoke_departed_consents`. It revokes
  the consents (and their refresh families) bound to that workspace and
  granted by that member (`bound_member`).
  - The access token dies at the next `Workspace<S>` membership check, at
    the latest when it expires 10 minutes later.
- `workspaces.workspace_deleting` → `identity.remove_workspace_clients`. It
  deletes the workspace's clients, revokes their families, and revokes
  consents bound to it.

## Endpoints and scopes

Protocol (`security(())`, client authentication is the protocol's):

| Path | Purpose |
| --- | --- |
| `GET /.well-known/oauth-protected-resource`, `…/api` | RFC 9728 metadata of the API |
| `POST /oauth2/register` | RFC 7591, public clients only, rate-limited |
| `POST /oauth2/token` | + `client_credentials`, `resource`; rate-limited per address |
| `GET /auth/consent/{request}` | what the consent screen shows (issuer session) |
| `POST /auth/consent/{request}` | allow (with a workspace) or deny (issuer session, CSRF, recent sign-in, rate-limited) |

API (`/api/v1/…`):

| Operation | Path | Scope |
| --- | --- | --- |
| `identity.list_consents` | `GET /identity/consents` | `identity:read` |
| `identity.revoke_consent` | `DELETE /identity/consents/{id}` | `identity:write` |
| `identity.list_clients` | `GET /w/{workspace_id}/identity/clients` | `identity:clients`, owner/admin |
| `identity.register_client` | `POST /w/{workspace_id}/identity/clients` | `identity:clients`, owner/admin, step-up |
| `identity.rotate_client_secret` | `POST /w/{workspace_id}/identity/clients/{id}/secret` | `identity:clients`, owner/admin, step-up |
| `identity.remove_client` | `DELETE /w/{workspace_id}/identity/clients/{id}` | `identity:clients`, owner/admin |

A member who isn't an owner or admin gets 403 `not_workspace_admin` on the
client operations. They can see the workspace, and the action is what is
refused.

**`identity:*` scopes are first-party only.**

- `identity:read`, `identity:write` and the new `identity:clients` manage a
  person's account: its ways in, connected apps and the workspace's clients.
- A third-party client that asks for one gets `invalid_scope`. The consent
  screen never offers one, and a workspace client can't be registered with
  one.
- The S2 review asked for this for `identity:write`. Read is included
  because a list of someone's ways in and connected apps is not an
  integration's business.

## Flows

### Finding a client

`Clients::find(db, id)` checks, in order:

1. the first-party clients in code;
2. the `clients` table, inside `begin_as_server`;
3. at `/oauth2/authorize` only, an `https` `client_id` with a path: the
   metadata document (below).

A removed client is unknown, and its refresh families were revoked when it
was removed.

### Authorization with consent

`/oauth2/authorize` validates the request as before, plus `resource` and the
client's scope ceiling. Then `complete` decides:

1. **No consent needed** (`campbooks-web`): issue the code, as today.
2. **A remembered consent covers it**:
   - the client's redirect URI is an exact `https` URI, not loopback and not
     a private-use scheme;
   - the account has an unrevoked, `remembered` consent for the client
     whose scopes include every requested scope;
   - no `prompt=consent`.

   Then issue the code bound to the consent's workspace, with no screen.
3. **Otherwise**:
   - if the issuer session's sign-in is older than ten minutes, send the
     person to sign in again (as `max_age=600`) and come back;
   - then store the request with `consent_due` and send the browser to
     `/consent?request=<id>`.

The sign-in callback and account creation reach step 3 through
`complete_stored` too. S2's sign-in code is not touched.

**`GET /auth/consent/{request}`** answers 404 unless both hold:

- the browser's issuer session is live;
- the request waits for consent.

It returns:

- the client: id, name, kind, `verified` (only workspace and first-party
  clients are), the redirect host, `loopback` (a loopback or
  private-use-scheme redirect), a logo for a workspace client;
- the scopes, each with the server's description;
- `binds_workspace`;
- the person's workspaces (from `Membership::workspaces_of`), with the one a
  workspace client belongs to marked as the only choice;
- the account's name and address.

**`POST /auth/consent/{request}`** takes `{ decision: "allow" | "deny",
workspace_id? }` and needs:

- the `X-Campbooks-CSRF` check;
- the issuer session;
- a sign-in in the last ten minutes, so a session that went stale while the
  screen sat open is refused with `step_up_required`;
- the per-address limit.

On `allow`:

- the workspace must be one the person belongs to, resolved through
  `Membership::resolve`; a workspace client may only be allowed in its own
  workspace;
- the consent is upserted, `remembered` by the rule above, with
  `bound_member` = the workspaces `UserId`;
- the request is consumed and a code issued, bound to the workspace;
- audit and `identity.consent_granted` are recorded.

`deny` consumes the request and answers the client's redirect with
`error=access_denied`. Both answer `{ redirect }`; the screen navigates
there.

### Native apps

`campbooks-native` is active. Its redirects are `http://127.0.0.1:<any>/callback`,
`http://[::1]:<any>/callback` and `com.notacamp.campbooks:/oauth/callback`.
Being first-party, it may ask for every API scope. It is a public client,
so its consent is never remembered (rule 2 above), whatever it asked
before.

The screen names "Campbooks for desktop and mobile" and says the token works
in all of the person's workspaces (`binds_workspace = false`): the app has a
workspace switcher like the web app. A Grant is recorded as a consent row
(not remembered), so the app shows under connected apps and can be revoked.

This closes RFC 8252 §8.6. A local process that starts an authorization for
`campbooks-native` gets a code only if the person reads a screen that names
the app and presses Allow after a recent sign-in.

### Workspace-bound tokens

- A code and its refresh family carry `bound_workspace_id`. Access tokens
  minted from them carry `workspace_id`.
- The in-process verifier (strict mode) keeps `workspace_id` on **user**
  tokens. An outside IdP's user claims stay ignored, as before, because a
  user may be able to edit them.
- `Workspace<S>` answers 404 when any token's `workspace_id` names another
  workspace. A user token without the claim (web, native) is unchanged.
- Routes outside a workspace (`/api/v1/workspaces`, held grants, ways in)
  stay as they are. A third party reaching them would need `workspaces:*`
  scopes, which list only the person's own data, or `identity:*` scopes,
  which it can't hold.

### client_credentials

- Only a confidential workspace client may use it, with `client_secret_basic`
  or `client_secret_post`.
- `scope` must be within the client's `allowed_scopes`; missing means all of
  them.
- The access token has `sub` = `azp` = `client_id` and `workspace_id` = the
  client's workspace. It has no refresh token and no ID token
  (RFC 6749 §4.4.3).
- `last_used_at` is touched at most once a minute.

### Resource indicators (RFC 8707) and audience

- The API's resource identifier is `<public origin>/api`. Every access token
  of the in-process issuer has `aud` = that URI. The verifier in
  `in_process` mode expects the same, and `CAMPBOOKS_AUTH__AUDIENCE` keeps
  meaning only what an outside IdP puts in `aud`.
- A `resource` other than the API's (authorize or token, at most one) is
  `invalid_target`. A refresh may only name the family's resource.

### Protected-resource metadata (RFC 9728)

- `GET /.well-known/oauth-protected-resource/api` (path-inserted form) and
  `GET /.well-known/oauth-protected-resource` (the root, for clients that
  don't insert) both return:
  - `resource`;
  - `authorization_servers` (the issuer);
  - `scopes_supported` (the third-party scopes);
  - `bearer_methods_supported: ["header"]`;
  - `resource_name`.
- When the issuer runs here, the server wraps the API routes in
  `auth::challenge_with_resource_metadata(url)`. Every `WWW-Authenticate:
  Bearer …` they answer (401, and 403 `insufficient_scope`) gains
  `resource_metadata="<url>"`.

### Dynamic registration (RFC 7591)

`POST /oauth2/register` takes JSON.

- `redirect_uris`: required, 1–10, each `https` with a host, or a loopback
  `http` URI: `127.0.0.1`, `[::1]` or `localhost`, any port, no fragment, no
  userinfo.
- `token_endpoint_auth_method`: `none` or missing; anything else is
  `invalid_client_metadata`.
- `grant_types` ⊆ {`authorization_code`, `refresh_token`} and
  `response_types` ⊆ {`code`}.
- `client_name`: 1–100 characters, defaulting to the redirect host.
- `scope`: within the third-party scopes.
- Logo and URIs are accepted and ignored.

The answer is 201 with `client_id` (`dcr_` + 128 random bits),
`client_id_issued_at` and the registered metadata. There is no secret and no
registration access token (RFC 7592 isn't offered). It is limited to 20
registrations per address per hour. A client unused for 30 days is pruned.

### Client ID Metadata Documents

A `client_id` that is an `https` URL with a non-root path is a metadata
document.

- On authorize, the row is fetched when missing or past
  `metadata_expires_at`.
- The document must:
  - be JSON;
  - have `client_id` exactly equal to the URL;
  - have `redirect_uris`, by the same rules as DCR;
  - not claim a secret or any auth method but `none`.
- `client_name` is shown with the host of the `client_id`.
- It is cached by `Cache-Control: max-age`, between 5 minutes and 24 hours
  (5 minutes when absent).
- A document that can't be fetched or is invalid makes the request
  `invalid_client`, shown and never redirected, because the redirect URIs
  aren't known.

**SSRF guard** (`upstream::fetch_guard`, new; nothing on `next` to reuse):

- `https` only:
  - no userinfo, no IP-literal host, default port;
  - at most 2048 characters.
- The host is resolved once. Every resolved address must be global, so the
  guard refuses:
  - loopback, private (RFC 1918), CGNAT `100.64/10`, link-local
    (incl. `169.254.169.254`), unspecified, broadcast, multicast,
    documentation, benchmarking and reserved ranges;
  - IPv6 unique-local, link-local, site-local and multicast;
  - IPv4-mapped and NAT64 forms of the above.
- The request goes to the checked address (`reqwest` `resolve`), so a second
  DNS answer can't swap it. The certificate is checked against the name.
- No redirects, 5 s overall, a 64 KiB body cap read incrementally, and
  `application/json` only.
- Tests replace the fetcher with an in-memory one. The address checks are
  unit-tested, including a name that resolves to loopback.

### Settings → Apps and integrations

One page, `/w/$workspaceId/settings/apps`, reached from the account menu:

- **Connected apps** (anyone): the person's consents, each with the app,
  whether it is verified, the workspace it may use ("All your workspaces"
  for native), what it may do, the date granted and the date last used,
  and **Revoke**, confirmed in place.
- **This workspace's apps** (owners and admins; members see a short note):
  - the workspace's clients, and **New app**: name, kind (server-to-server
    or sign-in app), scopes, redirect URIs for a sign-in app;
  - the secret is shown once in a callout with **Copy**, then never again;
  - **Rotate secret** (confirmed; shows the new secret once) and **Remove**
    (confirmed);
  - creating and rotating follow S2's step-up pattern: on
    `step_up_required` the page offers "Sign in again" and comes back.

### Consent screen (`/consent`)

The centred one-question layout of the sign-in screens (`FocusPage`, the
Visor):

- the app's name, with "Unverified" (Tag) for DCR and metadata-document
  clients and "Campbooks" for the native app;
- "wants to use your Campbooks account (ana@example.com)";
- what it may do, as a list of plain sentences, one per scope (translated
  by scope name; the server's English description is the fallback);
- the workspace picker (`ChoiceCards`, `↑↓`) when it binds one, or "all your
  workspaces" for the native app;
- "You'll be sent to `host`" with the redirect host in full. For a
  loopback or custom-scheme client: "an app on this device", and a warning
  Banner that it can act as you if you didn't just start this yourself;
- **Allow** (no shortcut) and **Deny** (`Esc`);
- errors in place: `step_up_required` offers "Sign in again", an expired
  request says start again from the app.

## Decisions and alternatives considered

The Rails app on `main` (Doorkeeper, client_credentials only, Settings → API
access) was read as prior art.

- **Copied on the merits:**
  - client credentials per workspace;
  - the secret shown once;
  - a settings page to create, rotate and revoke.
- **Not copied:**
  - BCrypt for secrets. A 256-bit random secret needs no slow hash, and
    SHA-256 lets the token endpoint look a client up in one query.
  - Doorkeeper's per-application owner and scopes columns: ours are a row
    under RLS.
  - Its lack of consent and DCR.

### 1. Making the native client safe

- **A. Consent for every authorization, after a recent sign-in (chosen).**
  - The person sees the app's name and what it may do, and presses Allow.
  - A malicious local process can still *start* an authorization, but it
    gets nothing unless the person, who signed in within ten minutes, agrees
    to a screen that says "an app on this device".
  - It costs one screen per native sign-in, which happens rarely (refresh
    tokens last 90 days).
- **B. Remember a native consent per device.** That means a device cookie
  or a key, but RFC 8252 §8.6 is exactly that anyone on the device can
  claim the loopback redirect. Remembering it brings back the silent code.
  Rejected.
- **C. Only claimed `https` redirects (app links) for native.** That is
  impersonation-proof on mobile, but desktop has no equivalent (loopback is
  RFC 8252's desktop answer), and the Tauri work (#17) would need a hosted
  association file per install. It may come later for mobile, on top of A.
- **D. A pairing code shown in the app and typed in the browser.** It is
  strong, but it is a device-flow-like UX nobody asked for, and the
  consent screen still needs to exist for third parties.

### 2. When a consent may be skipped

- **A. By redirect type (chosen).**
  - Remembered only for exact `https` redirects. A code there reaches only
    whoever controls that URL, i.e. the client that was consented to.
  - Loopback and private-use schemes (native apps, many MCP agents) are
    asked every time.
  - It is one rule, the same for workspace, DCR and metadata clients.
- **B. By client kind** (remember workspace clients, never DCR). That is
  simpler to state, but a DCR client with an `https` redirect is as safe to
  remember as a workspace one, and a workspace public client with a loopback
  redirect is not.
- **C. Never remember.** It is the safest, but every MCP reconnect with an
  `https` callback would prompt. The brief asks for remembered consents.

### 3. Where clients live

- **A. One `clients` table, forced RLS plus an authorization-server policy
  (chosen, as the S1a design sketched).** One lookup path for every kind. The
  management API is isolated by workspace like any workspace-owned table.
- **B. Workspace clients in one table (RLS) and registered clients in
  another (no RLS).** That avoids the second policy, but every lookup checks
  two tables, and pruning and revocation code is written twice.
- **C. Everything in code or config.** That only works for first-party
  clients.

### 4. The audience

- **A. `aud` = the API's resource URI, `<origin>/api` (chosen).** RFC 8707
  and RFC 9728 identify a resource by an absolute URI. MCP clients send it
  as `resource`, and the protected-resource metadata names it. A future MCP
  endpoint is a second URI.
- **B. Keep `aud` = `CAMPBOOKS_AUTH__AUDIENCE` (`campbooks-api`).** Nothing
  changes, but `campbooks-api` isn't a URI, so a conforming `resource` could
  never match it, and the metadata would name something else than the
  tokens.

Access tokens live ten minutes, so the switch is invisible: the next refresh
mints the new `aud`.

### 5. The person's workspaces on the consent screen

- **A. A kernel port method `Membership::workspaces_of` (chosen).** The
  consent screen is served by the issuer to a browser that may have no web
  app session (a native sign-in, an MCP agent), so the SPA can't ask
  `workspaces.list` through the BFF. The workspaces module already
  implements the port.
- **B. Read `workspaces.*` from identity.** That is a cross-schema read.
  Forbidden.
- **C. Let the person type a workspace id.** No.

### 6. Adding `resource_metadata` to challenges

- **A. A layer the server applies to the API routes when the issuer runs
  here (chosen).** The kernel's `Error` stays static, and the URL comes from
  the identity module.
- **B. Carry the URL in `TokenVerifier` and build it into `Error`'s
  response.** `IntoResponse` has no state, so the URL would need a
  thread-local or a global.

### 7. Dynamic registration

- **A. Open, public clients only, rate-limited, unverified, pruned
  (chosen).** MCP agents register without a human, and a registered client
  can do nothing without a person's consent, which it never skips for
  loopback redirects and always marks as unverified.
- **B. Require an initial access token.** Safer against spam, but no MCP
  agent can get one.

### 8. The SSRF guard

- **A. Hand-rolled: resolve, check every address, pin it, no redirects,
  small caps (chosen).** About 150 lines with table tests. The checks are
  the well-known ranges, and `reqwest`'s `resolve` pins the address.
- **B. A crate.** The candidates found are unmaintained or pull an HTTP
  stack we don't use. The S1a crate review found nothing suitable.
- **C. An egress proxy.** It is the most robust, but it is one more service
  for self-hosters.

## Rate limits

From `kernel::rate_limit` (per address unless noted):

- `identity.register`: 20 per hour.
- `identity.token`: 120 per minute. In-process requests (the BFF) are not
  counted.
- `identity.consent`: 30 per minute.
- `identity.metadata_document`: 30 metadata-document lookups per minute (each may be
  a fetch from the internet), so the server can't be made to fetch without end.

## Security notes

- Every refusal path is a named test:
  - unknown client;
  - wrong redirect;
  - `invalid_target`, `invalid_scope` for `identity:*`;
  - consent without CSRF, without a session, without a recent sign-in, for
    another person's workspace, or for a workspace client in another
    workspace;
  - deny;
  - remembered consent skipped for loopback;
  - `client_credentials` from a public client, from a DCR client, with a
    wrong or rotated secret;
  - a bound token on another workspace (404);
  - DCR with a non-`https` redirect, a secret method or rate-limited;
  - a metadata document with a mismatched `client_id`, too big, a
    redirect, or a private address.
- No secret, token, client secret, document body or address is logged.
  Client ids and kinds are.
- What's left for #57:
  - the OIDF conformance suite on staging;
  - a review of the SSRF guard against DNS rebinding with IPv6 and NAT64
    prefixes beyond `64:ff9b::/96`;
  - per-client limits on the token endpoint.

## After the security review

- **Bound tokens outside a workspace.** `workspaces.list` shows a bound
  token its workspace only. `workspaces.create` and
  `workspaces.accept_invitation` answer 404 to it. Every `identity.*` API
  route takes the kernel's new `Unbound<S>` (404 for a bound token).
  `workspaces:write` and `workspaces:admin` join `identity:*` and
  `admin:access` as Campbooks' own. Self-registered clients get read scopes
  only when they name none. A server end-to-end test walks each route.
- **Consent screen.**
  - Focus starts on Deny, and no workspace is preselected.
  - Allow has no shortcut and works only after the page has been visible
    and focused for one second. Hiding the page or leaving the window
    starts the wait again.
  - Clickjacking by keystroke is closed.
- **SSRF guard.** No proxy, and `::ffff:0:0:0/96` is refused.
- **Abuse.**
  - IPv6 is counted per /64 (as before).
  - Instance-wide hourly caps on registrations (300) and document fetches
    (600).
  - Failed documents aren't fetched again for 5 minutes (per process).
- **Spoofing.**
  - Names lose control, format and bidi characters.
  - "Campbooks" and "Not A Camp" are reserved: case-, spacing- and
    look-alike-insensitive. A self-registered app shows its host instead; a
    workspace client is refused with `client_name_reserved`.
  - "Verified" now means Campbooks' own. A workspace's app is named by its
    workspace.
- **Revocation.**
  - A code or refresh token is redeemed only while a consent for the same
    workspace covers its scopes.
  - Replacing a consent ends the old tokens that were bound to another
    workspace or to more scopes.
  - Remembered consents cover their own redirect URI only, for 90 days.
  - Access-token `jti` tracking on removal is left for #57 (they live ten
    minutes).
