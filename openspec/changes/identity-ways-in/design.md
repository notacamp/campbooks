# Design

## Context

ADR 0014 and the S1a design (`openspec/changes/archive/2026-10-13-campbooks-identity-s1a/design.md`)
fixed the shape: `identity` runs every provider round trip ("one front door")
with a purpose (`sign_in`, `link`, `connect`, `reconnect`), links only by a
round trip completed while signed in, refuses an identity that belongs to
another account, never removes the last way in, and needs a recent sign-in
for sensitive changes. This document decides what those left open: how a
signed-in round trip is started and bound, how Microsoft's and Zoho's
issuers are checked, how `connectors`' consent becomes a delegate, and the
settings screen.

Coordination with the sibling slices (agreed with S3 and S4):

- **S3 (passkeys)** builds the step-up check
  (`identity::domain::step_up::require_recent`, kernel
  `Principal.auth_time`, BFF `/bff/login?max_age=`) and
  `identity::domain::ways_in::lock_and_count`, which locks the account row
  and counts provider identities plus passkeys. This slice uses both.
  S3 owns `/api/v1/identity/passkeys` and exports `<PasskeysSection/>`,
  which this slice's page renders.
- **S4 (clients)** builds the kernel's per-IP rate limiter; this slice puts
  limits on `/auth/start`, the callbacks and `identity.start_link`.

## Data ownership

Schema **`identity`**, migration `<ts>_identity__ways_in.sql`:

```text
link_tickets   ticket_hash bytea PK · account_id → accounts · provider ·
               purpose (link|connect|reconnect) · workspace_id? ·
               connection_id? · login_hint? · return_to · created_at ·
               expires_at (2 minutes)
upstream_flows + account_id? → accounts · workspace_id? · connection_id?
```

`identity.prune` also deletes expired tickets. A ticket's `login_hint` is
the mailbox address of the connection being reconnected, kept for at most
two minutes and never logged.

Schema **`connectors`**, migration `<ts>_connectors__drop_authorizations.sql`:
`connectors.authorizations` (the consent started by `start_connection`) is
dropped; `identity.upstream_flows` holds the round trip now.

No other module's data is read: `identity` learns whether a connection may
be reconnected, and by whom, through the grant-intake port.

## Events

Published by `identity`, instance-wide, ids and names only:

- `identity.way_in_added` v1 `{ account, method: "provider", provider, via: "link"|"connect" }`
- `identity.way_in_removed` v1 `{ account, method: "provider", provider }`

`method` leaves room for S3's passkeys (`"passkey"`, `provider: null`).
Nothing new is consumed.

## Endpoints and scopes

| Operation | Path | Scope |
| --- | --- | --- |
| `identity.list_ways_in` | `GET /api/v1/identity/ways-in` | `identity:read` |
| `identity.remove_way_in` | `DELETE /api/v1/identity/ways-in/{id}` | `identity:write` + step-up |
| `identity.start_link` | `POST /api/v1/identity/links` | `identity:write` (+ step-up for `link`, `connect`; + membership for `connect`, `reconnect`) |
| `connectors.start_connection` | unchanged path | `connectors:write`, delegates to the front door |
| `connectors.complete_connection` | removed | |

The protocol routes keep their paths: `/auth/start/{provider}` gains the
`link` parameter; `/auth/callback/{provider}` gains Zoho's `location` and
`accounts-server`. `{provider}` is now `google`, `microsoft` or `zoho`.

## Flows

### Starting a signed-in round trip

1. The SPA calls `identity.start_link {purpose, provider?, workspace_id?,
   connection_id?, return_to?}` through the BFF.
   - `link`: `provider` required and configured; step-up.
   - `connect`: `provider` and `workspace_id`; step-up; the caller must be a
     member (`Membership` port, else 404); the provider must have mailbox
     scopes (`GrantIntake::mailbox_scopes`), else 409 `mailbox_unsupported`.
   - `reconnect`: `workspace_id` and `connection_id`; the grant-intake port's
     `reconnect_target` says which provider and address, and answers 404
     unless the caller granted that connection. No step-up: the provider
     must return the same account (`connectors` refuses `account_mismatch`),
     so nothing new becomes a way in.
2. The answer is `{ url: "/auth/start/{provider}?link=<ticket>", expires_at }`:
   a random 256-bit ticket, stored hashed, single use, two minutes.
3. The browser navigates there. `/auth/start` takes the ticket and requires
   the browser's issuer session (`__Host-campbooks_auth`) to belong to the
   ticket's account. Otherwise nothing starts and the browser goes back to
   `return_to?link_error=sign_in_again` (or `link_expired` for an unknown or
   used ticket).
4. A flow is stored as for sign-in (state, nonce, sealed PKCE verifier,
   browser cookie), now with purpose, account, workspace and connection. The
   provider is asked with `prompt=consent` (a refresh token always comes
   back when offline access is asked) and, for `reconnect`, `login_hint`.
   Scopes: identity only for `link`; identity, mailbox and offline for
   `connect` and `reconnect`.

### Finishing it

The callback validates as for sign-in (state, browser, provider error,
RFC 9207 `iss`, code exchange with PKCE, ID token). Then, by
`(provider, issuer, subject)`:

- **This account's** way in: refreshed (`last_used_at`).
- **Nobody's**: inserted with `linked_via = link` (or `connect` for
  `connect`/`reconnect`), audited, `identity.way_in_added` published.
- **Another account's**: `link_error=way_in_belongs_to_another_account`.
  Nothing is merged or moved, and no connection is made.

For `connect` and `reconnect`, the grant (with its refresh token, and
`extending` for `reconnect`) goes to `GrantIntake::adopt`. Its refusals
(`account_mismatch`, `account_connected_by_another_member`,
`consent_incomplete`) come back as `link_error=<code>`; the way in stays
linked, because the round trip proved ownership. On success the browser
lands on `return_to?linked=<provider>` or `?connected=<connection id>`.

### Removing a way in

`identity.remove_way_in` needs step-up. In one transaction it calls
`ways_in::lock_and_count` (locks the account row; providers plus passkeys)
and refuses 409 `last_way_in` when the count is one; otherwise it deletes the
provider identity (404 when it is not the caller's), audits, and publishes
`identity.way_in_removed`. It never disconnects a mailbox. A grant held since
sign-up for that identity goes with it (the cascade S1a set up); the
confirmation says so. Sessions signed in with it keep running.

### Microsoft

- Endpoints `https://login.microsoftonline.com/common/oauth2/v2.0/{authorize,token}`,
  keys at `…/common/discovery/v2.0/keys`; `response_mode=query`.
- **Issuer.** The ID token's `tid` is read from the unverified payload only to
  compute the expected issuer, and must be a lowercase GUID. The token is then
  validated (signature from the common key set, audience, `azp`, expiry,
  nonce) with `iss` required to equal
  `https://login.microsoftonline.com/{tid}/v2.0`, and the validated `tid` must
  equal the one used. The identity key is `(microsoft, that issuer, sub)`.
  Personal accounts have their own fixed tenant id and pass the same rule.
- **Email.** `email` (else `preferred_username`) is kept for display only.
  It counts as verified only when the optional claim `xms_edov` is `true`;
  Microsoft has no `email_verified`, and any such claim is ignored. Linking
  never looks at email anyway.
- Offline access is the `offline_access` scope, asked only with mail
  scopes. Microsoft has no mail adapter yet, so a Microsoft sign-in keeps no
  token.

### Zoho

- The browser goes to `https://accounts.zoho.com/oauth/v2/auth`, which serves
  every data centre. Zoho's redirect carries `location` and
  `accounts-server`.
- `accounts-server` must be exactly one of `https://accounts.zoho.com`,
  `.eu`, `.in`, `.com.au`, `.jp`, `.ca`, `.sa`, `.uk`; anything else is
  refused (`login_failed`) **before** any request, so the code and the client
  secret can only go to a Zoho accounts server. The code is exchanged at
  `<accounts-server>/oauth/v2/token`, the ID token's `iss` must equal that
  server, and its keys come from `<accounts-server>/oauth/v2/keys` (one key
  cache per data centre). The identity key is `(zoho, accounts server, sub)`.
- `access_type=offline` only with mail scopes (none until #74), so no Zoho
  refresh token is minted and dropped (Zoho keeps at most twenty per person
  and app).
- One Zoho client serves every data centre when "multi-DC" is enabled in
  Zoho's API console; `deploy/README.md` says so.

### Settings

`CAMPBOOKS_MICROSOFT__CLIENT_ID` / `__CLIENT_SECRET[_FILE]` and
`CAMPBOOKS_ZOHO__CLIENT_ID` / `__CLIENT_SECRET[_FILE]`, read like Google's. A
provider with no client id is not offered.

- `CAMPBOOKS_MICROSOFT__TENANT` (default `common`): `organizations` (work and
  school only), `consumers` (personal only) or one tenant id. An install whose
  Microsoft app is registered for work accounts only runs on
  `organizations` until its registration is switched. The issuer rule is the
  same for all; with a tenant id, `tid` must also equal it.
- `CAMPBOOKS_MICROSOFT__REDIRECT_PATH` and `CAMPBOOKS_ZOHO__REDIRECT_PATH`
  (default `/auth/callback/<provider>`): the path sent as `redirect_uri`, for
  an OAuth app registered on another path that the reverse proxy rewrites to
  `/auth/callback/<provider>` (the same override Google gets in #94). It must
  be a plain absolute path. `…__BASE_URL` points a provider at
a fake (laid out like the real one) and is refused outside
`CAMPBOOKS_ENV=development`; for Zoho it replaces the allowlist with that one
server.

### `connectors`' consent through `identity`

- `connectors.start_connection` keeps its path and request. It checks what
  it owns (a member, not a client; capabilities offered; the connection to
  extend is the caller's and the same provider) and then calls the kernel
  port `ConsentDoor::start` with the caller's principal, which runs
  `identity.start_link` (`connect`, or `reconnect` when a connection is
  named). The answer's `authorize_url` is the front-door URL. With no
  in-process issuer (an install still on Rauthy), it is 409
  `provider_not_configured`.
- `connectors.complete_connection`, the `authorizations` table, the
  `/connections/google/callback` redirect URI and the Google client's
  authorize/exchange/userinfo code are removed: the callback is
  `identity`'s, and the grant arrives through `GrantIntake::adopt`, whose
  `ProviderGrant` gains `extending`. `create_or_refresh_connection` stays the
  one seam.
- `GrantIntake::reconnect_target(subject, workspace, connection)` answers
  the connection's provider and address when the caller granted it, else
  404.

## Screens (clients/app, module `identity`)

- **Settings → Ways to sign in** (`/w/$workspaceId/settings/sign-in`,
  next to Members; ways in belong to the person, the workspace frame is
  where settings live):
  - one row per provider way in: the provider's name, the address, when it
    was added and last used;
  - **Remove** opens an inline confirmation ("You'll no longer be able to
    sign in with …; your mailbox stays connected"). A `last_way_in` refusal
    is explained in place;
  - **Add a way in**: one button per configured provider not yet added;
  - **Mailboxes to reconnect**: this workspace's connections of yours that
    need re-authorisation, each with Reconnect;
  - when an action needs a recent sign-in (`step_up_required`), the page says
    so and offers "Sign in again" (`/bff/login?max_age=600&return_to=…`);
  - results come back as `?linked=`, `?connected=` or `?link_error=` and are
    shown as a message, then cleared from the URL;
  - **extension point**: the page takes `sections`, rendered below the
    provider list; the module passes S3's `<PasskeysSection/>`.
- **Sign-in screen** (#455): Microsoft and Zoho buttons appear when
  `GET /auth/requests/{id}` lists them, in the server's order, with
  `1`/`2`/`3`.

All copy in en, pt-PT, es-ES and fr; `lib/ui` components and tokens only;
keyboard hints from the registry; light and dark; 375px and desktop.

## Security posture

- **Login CSRF on linking.** A link starts only from an authenticated API
  call (bearer token through the BFF, which needs its CSRF header), and its
  ticket works only in a browser whose issuer session is the same account.
  An attacker's ticket opened in a victim's browser, or a victim's ticket in
  an attacker's browser, starts nothing.
- **Mix-up.** One redirect URI per provider; the flow's provider must match
  the callback path; `iss` checked when sent.
- **Zoho SSRF / exfiltration.** The accounts server comes from the
  allowlist only; no request goes anywhere else.
- **Microsoft multi-tenant issuer.** Bound to the token's own `tid` after
  signature validation; a token from tenant A cannot claim tenant B.
- **Unverified email.** Microsoft's address is never verified unless
  `xms_edov`; no path links by email.
- **Tokens on refusal are dropped, not revoked.** Revoking a Google token
  revokes the person's whole grant to the app, including the mailbox another
  account connected with it: a stranger could cut someone's mailbox by
  linking their account elsewhere. Same reasoning as S1a's held grants.
- **Step-up** on link, connect and remove; the last way in can't go (row
  lock, so two removals can't race past it).
- **Rate limits** (S4's kernel limiter): `/auth/start` and the callbacks per
  IP, `identity.start_link` per account.
- **Logs** carry provider names, purposes and ids; never codes, tokens,
  tickets, addresses or the `accounts-server` value.

## Decisions and alternatives considered

Prior art in the Rails app on `main` (`Auth::OauthSignIn`): it links a
mailbox to the signed-in user by **verified email**, and connects mail
through a separate per-provider controller. Neither is copied: linking is by
round trip only, and there is one front door.

### 1. Binding a signed-in round trip to its account and browser

- **A. API-issued ticket + issuer-session match (chosen).** The operation
  is authenticated, scoped and step-up checked like any other; the browser
  only carries an opaque, single-use, two-minute ticket, which needs the
  same account's issuer session to start. One small table.
- **B. `/auth/start?purpose=link`, trusting the issuer session cookie
  alone.** No table, but the session cookie is `SameSite=Lax`, so any site
  can start a link with a top-level GET. With a provider login CSRF, the
  attacker's provider account would be linked to the victim's Campbooks
  account: an account takeover. Rejected.
- **C. The BFF starts the round trip** (it knows the session). The BFF would
  then own provider flows, which ADR 0014 gives to `identity`, and native
  apps would need a second path. Rejected.

### 2. Microsoft's issuer

- **A. Expected issuer from the token's `tid`, then a normal validation with
  that issuer (chosen).** One key cache for `common`; the kernel's verifier
  gains "validate against this issuer, and give me these extra claims", a
  small generalisation, and no check is skipped.
- **B. Validate without an issuer and compare afterwards.** Same result, but
  it needs a "skip issuer" switch in the kernel verifier, an easy thing to
  misuse elsewhere.
- **C. One verifier per tenant**, created on first sight. A cache keyed by an
  attacker-chosen value, and a JWKS fetch per tenant. Rejected.

### 3. Zoho data centres

- **A. A fixed allowlist of the eight accounts servers, one key cache each
  (chosen).** Every data centre's users can sign in; nothing goes to an
  address the callback made up.
- **B. One data centre per install** (`CAMPBOOKS_ZOHO__DATA_CENTRE`). Simpler,
  but a hosted instance would turn away most of the world's Zoho users.
- **C. Trust `accounts-server` if it ends in `zoho.*`.** Suffix matching is
  how SSRF guards fail. Rejected.

### 4. How `connectors`' consent becomes a delegate

- **A. `connectors.start_connection` calls a kernel port `identity`
  implements; `complete_connection` is removed (chosen).** The public
  operation keeps working for any client, `identity` stays the one front
  door, and `create_or_refresh_connection` stays the one seam. The removed
  operation had no caller: the provider now redirects to `identity`'s
  callback, so nothing could complete it.
- **B. Remove both; clients call `identity.start_link` directly.** Less
  code, but a published operation disappears for no gain.
- **C. Keep `connectors`' own flow next to `identity`'s.** Two redirect URIs
  per provider, two implementations of state and PKCE, and connected
  mailboxes that are not ways in. ADR 0014 rejected it.

### 5. Step-up on reconnect

- **A. Not needed (chosen).** The provider must return the very account of
  the connection (`account_mismatch` otherwise), so no new way in can appear,
  and a mailbox that stopped syncing is fixed in one click.
- **B. Needed, like connect.** Uniform, but it adds a full sign-in before
  every reconnect for no security gain.

### 6. Provider code shape

- **A. One `Upstream` client with per-provider data (endpoints, issuer
  rule, offline mechanism), chosen.** The three providers differ in a few
  parameters and the issuer check; the round trip, state and PKCE are one
  implementation.
- **B. A trait per provider.** Three implementations of the same HTTP calls;
  more code to review for the same behaviour.

### 7. Offline access without a mail adapter

- **A. Ask for offline access only with mailbox scopes (chosen).** A
  Microsoft or Zoho sign-in mints no refresh token we would throw away (Zoho
  caps them per person).
- **B. Always ask, as S1a did for Google.** Harmless for Google, wasteful and
  capped for Zoho.

## Risks

- **Zoho and Microsoft are verified against fakes only** (in-process
  `TestIdp` in tests, dev-only fakes in the recording); a live check with real
  apps is a follow-up before #57, like Zoho's calendar client on `main`.
- **Step-up adds a sign-in before linking** when the session is older than
  ten minutes. The page explains why, and comes back to where it was.
- **Removing a way in drops its held grant.** Said in the confirmation;
  the mailbox can still be connected with a new consent.
