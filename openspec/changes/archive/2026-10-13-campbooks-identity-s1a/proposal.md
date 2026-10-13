# Proposal

## Why

The owner decided that Campbooks owns identity end to end and that Rauthy
goes (ADR 0014, campbooks-cloud#75):

- the Google, Microsoft or Zoho account you first sign in with is also your
  first mailbox, in one consent;
- every mailbox you own is a way in to the same account;
- accounts are linked by proven ownership, never by email;
- Campbooks issues every token;
- the sign-in screens are Campbooks screens.

ADR 0014 records the design and the slices. This change is **slice S1a**
(campbooks-cloud#77): the issuer itself, signing in with Google, the
held mailbox grant and its hand-off to connections, and the web BFF signing
in against the issuer instead of Rauthy. Rauthy keeps working, untouched,
for installs that name it, until slice S5 removes it.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/77
(parent: https://github.com/notacamp/campbooks-cloud/issues/75).

**Modules touched**
- New module **`identity`** (`crates/modules/identity`, schema `identity`),
  depending on the kernel only.
- **kernel**:
  - `auth::KeySource`: a verifier reads an in-process issuer's keys;
  - `connections::GrantIntake` + `ProviderGrant`: the port a mailbox grant
    crosses;
  - `crypto::derive_secret`;
  - `IdToken` carries `email_verified` and `auth_time`;
  - `TestIdp` can play an upstream provider.
- **connectors**: implements `GrantIntake` over
  `create_or_refresh_connection`.
- **server**:
  - composes `identity` and mounts its protocol endpoints;
  - registers the in-process verifier and the grant intake;
  - the BFF signs in against the in-process issuer (derived secret, requests
    through its router).

No cross-module dependency is added: `identity` reaches `connectors` through
the kernel port, so `architecture.toml` is unchanged.

## What Changes

- **Issuer**, active when `CAMPBOOKS_AUTH__ISSUER` is unset (or names this
  instance) and `CAMPBOOKS_SECRET_KEY` is set:
  - discovery (OIDC and RFC 8414) and `/oauth2/jwks`;
  - `/oauth2/authorize`: code + PKCE S256 only, single sign-on, `prompt`
    and `max_age`, RFC 9207 `iss`;
  - `/oauth2/token`: authorization_code and refresh_token, with rotation and
    reuse detection;
  - `/oauth2/revoke` and `/oauth2/end_session`.
- **Keys**: Ed25519, generated on first use, sealed at rest, published a week
  ahead, rotated every 30 days, retired after two more days.
- **First-party clients** `campbooks-web` and `campbooks-native`, defined in
  code and needing no provisioning.
- **Google sign-in** (`/auth/start/google`, `/auth/callback/google`), the
  new-account step (`GET`/`POST /auth/new-account`), and what the sign-in
  screen shows for a waiting request (`GET /auth/requests/{id}`).
- **Held mailbox grants** and the API operations `identity.list_held_grants`,
  `identity.adopt_held_grant` and `identity.discard_held_grant`.
- **BFF on the in-process issuer**; logout ends the issuer session with no
  confirmation page.
- **Docs**: `deploy/README.md` → Built-in sign-in, `.env.example`,
  architecture, domain map, CHANGELOG.

## Non-goals

- The sign-in and new-account **screens** (S1b): until they land, the
  issuer's redirects to `/sign-in` and `/sign-in/new` reach the web app's
  fallback.
- **More ways in** (S2): Microsoft and Zoho, linking and removing ways in,
  connecting another mailbox through `identity`, step-up.
- **Passkeys** (S3).
- **Native sign-in end to end, third-party clients, consent, DCR, client
  metadata documents, RFC 8707 resources and RFC 9728 metadata** (S4). The
  native client is defined here, but the Tauri side is #17.
- **Removing Rauthy** (S5): compose, `deploy/idp`, its docs and settings stay.
- **Rate limits** on the sign-in and token endpoints: they need a trusted-proxy
  setting to know a client's address behind a reverse proxy. They are a
  follow-up before security review #57.
- Revoking held grants at the provider: Google revokes every token of an
  account's grant to an app at once, which would also cut a mailbox
  connected through that app; held grants are deleted instead.
- Tracking access-token revocation (`jti`): access tokens live ten minutes.
