# Design: instance apps

## Data ownership

Everything is the `identity` module's, in schema `identity`:

- `identity.clients` gains the kind `instance` (no workspace, a
  `secret_hash`, `auth_method = 'client_secret_basic'`, at least one
  redirect URI) and the column `post_logout_redirect_uris` (instance apps
  only, by a check). The checks naming the kinds are widened in the
  migration; the unnamed "only a workspace client has a secret" check is
  found by its columns (`kind`, `auth_method`) and replaced by a named one.
- Consents (`identity.consents`) and refresh families
  (`identity.refresh_families`) are the existing ones, keyed by client id.
- The security record is `identity.audit_events` (account `NULL`, metadata
  `{client, via: "cli", …}`: ids and counts only).

No new table, no new scope, no new resource type (the CLI is not an
operation), no cross-module dependency. The server's CLI calls
`campbooks_identity::domain::instance_apps`, as it already calls
`campbooks_admin::cli` and `identity::domain::accounts`.

## Who may change an instance app

Only the migration login, from `campbooks-server admin apps`:

1. No route of either API, nor the issuer's protocol router, writes one.
   `/oauth2/register` only ever creates `dynamic` clients and
   `identity.register_client` only `workspace` ones; their SQL names the
   kind.
2. A trigger (`identity.instance_apps_from_the_cli`) on insert, update and
   delete refuses any change to a row whose old or new kind is `instance`
   unless the current role may use the table owner's rights
   (`pg_has_role(current_user, owner, 'USAGE')`), with one exception: an
   update that changes `last_used_at` only (the token endpoint's
   `clients::touch`). The runtime role is not a member of the owner (the
   server refuses to start otherwise, `RoleReport::owns_policy_tables`), so
   a SQL injection or a bug in a request handler cannot add an instance app,
   widen its ceiling, add a redirect URI, set its secret or remove it.
   Row-level security cannot express this here: `identity.clients` forces
   RLS on its owner too, and the owner is a member of the runtime role, so
   a `TO campbooks_app` policy would also bind the CLI.
3. The CLI's transactions set `campbooks.identity_role =
   authorization_server` (`clients::as_server`), the policy the issuer
   already uses to see clients of no workspace.

Alternatives considered: a separate table for instance apps (every lookup
by client id would have to read two tables, and consents/refresh families
already key on the id); config-file clients like the first-party ones
(secrets would sit in the environment in clear, and rotating would need a
restart); an admin-API endpoint behind `Admin<super>` (the owner ruled out
any HTTP path for this, as for admin profiles).

## Protocol

- **Lookup.** `Clients::find` loads it like any stored client; the kind is
  never switched off by the self-registration flags.
- **Redirects.** `Client::accepts_redirect` matches an instance app's
  redirect URIs exactly, without the loopback any-port rule. The CLI keeps
  only canonical URIs (`Url::parse(uri).as_str() == uri`), so the exact
  string the app sends is the one the operator saw.
- **Scopes.** An instance app's `allowed_scopes` is its whole ceiling, OIDC
  scopes included. `Client::may_ask_for` admits a scope only when it is in
  the ceiling and is an OIDC scope or a catalogue scope a third party may
  hold (so a ceiling stored before the rules tighten cannot escalate).
  `authorize::allowed_scope` drops, for an instance app, every scope it may
  not ask for (intersection) instead of answering `invalid_scope`; an empty
  intersection is `invalid_scope`. `admin:access` is dropped for every
  non-first-party client, as before. The token response's `scope` and the
  consent screen list exactly what is granted.
- **PKCE and authentication.** Unchanged code paths: S256 is required for
  every client at `/oauth2/authorize` and the verifier at the token
  endpoint; an instance app authenticates with `Authentication::Hashed`
  (constant-time comparison of SHA-256 digests) via Basic or the form, and
  a request without its secret is `invalid_client`.
- **Consent and binding.** `needs_consent()` and `binds_workspace()` are
  true (it is not first-party), so the consent screen, the step-up window,
  the workspace pick, the remembered consent (90 days, exact `https`
  redirect), `consents::covering` on every code and refresh, the `cns`
  claim and the workspace-bound token rules of S4 apply unchanged.
- **Logout.** `/oauth2/end_session` accepts a hint whose `aud` is a
  first-party client or an instance app, and redirects only to one of that
  client's `post_logout_redirect_uris` (exact match), appending `state`.
  It ends the issuer session the hint names and its refresh tokens, as
  RP-initiated logout does for the web app: signing out of Applicamp signs
  that browser out of the issuer too.
- **Removal.** `apps remove` deletes the client, revokes its live consents
  (so its access tokens, which carry `cns`, stop at once) and its refresh
  families (`client_removed`); waiting codes find no client.
- **Discovery** is unchanged: `registration_endpoint` stays absent unless
  `CAMPBOOKS_OAUTH__DYNAMIC_REGISTRATION` is on.

What an app such as Applicamp configures: issuer `CAMPBOOKS_PUBLIC_URL`
(discovery at `/.well-known/openid-configuration`), authorization endpoint
`/oauth2/authorize`, token endpoint `/oauth2/token`, JWKS `/oauth2/jwks`,
end-session endpoint `/oauth2/end_session`, `client_secret_basic` (or
`client_secret_post`), response type `code` with PKCE S256, and the scopes
it was registered with (e.g. `openid profile email offline_access
conversations:read conversations:write conversations:send schedule:read
schedule:write`).

## The consent screen

`ConsentClient.kind` is `instance`; `verified` stays `false` (it is not
Campbooks' own). The web client shows the tag "Added by this server" and
"Added to this Campbooks by the people who run it." instead of
"Unverified", a happy visor and no "anyone can register an app" warning;
the workspace pick and everything else are as for any other app. Connected
apps shows the same tag. New strings in en, pt-PT, es-ES and fr, plus the
missing `schedule:read` / `schedule:write` scope sentences.

## Events

- Domain events: none published or consumed (see Non-goals).
- Tracked events (plan version 16, patch `0016-instance-apps.yaml`), all
  instance-wide, actor `system` with the command's name (`admin`), tracked
  in the transaction of the change after it:
  - `identity.instance_app_registered` v1: `client` (`identity.client`
    snapshot: id, kind `instance`, confidential, scope count, redirect
    count, times; never the name, secret, scopes or URIs);
  - `identity.instance_app_secret_rotated` v1: `client`, afterwards; an
    unknown id tracks nothing;
  - `identity.instance_app_removed` v1: `client_id`, `consents_revoked`,
    `refresh_tokens_revoked`; an unknown id tracks nothing.
- Changed: the enum `identity.client_kind` gains `instance`; every
  client-id pattern (the `identity.client` and `identity.consent` records'
  `client_id`, and `client_id` of `identity.client_removed`,
  `consent_denied`, `refresh_reuse_detected`, `refresh_token_revoked`,
  `tokens_issued`) gains `app_[A-Za-z0-9_-]{22}`. Both are additive: every
  value valid before stays valid, so no event changes version.
  `kernel::tracking::minted_client_id` accepts `app_` ids, so an instance
  app's id may appear in the envelope's `client` actor.
- Coverage rows: none. The CLI is no operation, job, schedule or handler
  (as for `admin grant|revoke`, plan version 8); no operation is added.

## Scopes

No endpoint is added. Existing endpoints keep their scopes
(`identity:read` to list consents, `identity:write` to revoke one).

## Risks

- An instance administrator with shell access could register an app with a
  broad ceiling; that is within their power anyway (they hold the database
  owner), and every person still sees and allows the scopes one by one.
- The `pg_has_role` check assumes the runtime role is not a member of the
  table owner, which the server already checks at start.
