# Instance apps: CLI-registered OAuth clients for first-party SSO

Module: `identity` (the authorization server's clients, consent and
logout), with the server's admin CLI (`crates/server/src/admin_cli.rs`),
the kernel's tracking context (which client ids may be recorded), the
tracking plan (`tracking/`) and the web client's consent screen and
connected apps (`clients/app/src/modules/identity`). Tracking:
https://github.com/notacamp/campbooks-cloud/issues/369. First client:
Applicamp (notacamp/applicamp#8).

## Why

Campbooks Next's issuer already speaks OpenID Connect (code + PKCE,
confidential clients, consent), but production has no way to register an
outside **web** app that signs people in: a workspace's own clients are
`client_credentials` or public sign-in apps bound to one workspace, and
dynamic registration and client metadata documents are off by default (as
they should be). The operator's own other apps, Applicamp first, need a
confidential client of the whole instance, with exact redirect URIs, a
scope ceiling and a place to return to after logout, that nobody can
register or widen over HTTP.

## What Changes

- A new client kind, **instance apps** (`identity.clients.kind =
  'instance'`, ids `app_` + 16 random bytes): confidential, authorization
  code with PKCE S256 required, `client_secret_basic` or
  `client_secret_post` at the token endpoint, refresh tokens rotated like
  every other client's when `offline_access` is granted.
- Registered, listed, rotated and removed **only** from the server's CLI,
  on the migration login, like admin profiles (ADR 0015):
  `campbooks-server admin apps add --name … --redirect-uri … [--redirect-uri …]
  [--post-logout-redirect-uri …] --scopes …`, `apps list`,
  `apps rotate-secret <client_id>`, `apps remove <client_id>`. No HTTP
  endpoint does any of this, ever; a trigger refuses the runtime role any
  change to an instance app except recording when it last got a token.
- Validation: redirect URIs are exact strings, `https` with a host
  (loopback `http` only on a development instance), in canonical form, no
  fragment or userinfo, 1 to 10, matched exactly (no loopback port leeway);
  post-logout redirect URIs the same, 0 to 10; the scope ceiling holds only
  `openid profile email offline_access` and the API scopes a third party may
  hold (never `identity:*`, `workspaces:write`, `workspaces:admin`,
  `admin:access`); names go through the look-alike checks
  (`client_names.rs`).
- Protocol: an authorization asking above the ceiling gets the
  intersection (nothing left: `invalid_scope`). The ID token carries `sub`,
  `email`, `email_verified`, `name` and `nonce` as for the BFF. Tokens are
  workspace-bound and stand on a consent, exactly like other third-party
  user tokens.
- Consent: the consent screen, once per person (remembered for the exact
  `https` redirect, as for any app), in en, pt-PT, es-ES and fr, with the
  app shown as added by this server (kind `instance`) instead of
  "Unverified"; listed in Connected apps, where the person can revoke it.
- RP-initiated logout: `/oauth2/end_session` accepts an instance app's ID
  token hint and returns to one of its registered
  `post_logout_redirect_uris`, matched exactly.
- Audit: `identity.audit_events` rows (`instance_app_registered`,
  `instance_app_secret_rotated`, `instance_app_removed`) and three tracked
  events (plan version 16, `tracking/patches/0016-instance-apps.yaml`);
  the client kind `instance` and the `app_` ids join the plan's client-id
  patterns and `kernel::tracking::minted_client_id`.
- Migration `20261109010000_identity__instance_apps.sql`.

## Non-goals

- Any HTTP endpoint or admin-dashboard panel to register, rotate, list or
  remove instance apps.
- Changing discovery: `registration_endpoint` stays absent unless dynamic
  registration is turned on, as today.
- First-party treatment: an instance app is still a third party to the
  person (consent screen, one workspace, no `identity:*` or `admin:access`).
- Logos, editing an app in place (remove and add again), or secrets with an
  expiry.
- Domain events for the CLI's changes (no module consumes client events
  today; the security record is `identity.audit_events` and the tracked
  events).
