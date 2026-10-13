# Proposal

## Why

The owner's requirements for sign-in (campbooks-cloud#75):

- The Google, Microsoft or Zoho account you first sign in with **is also
  your first mailbox**, granted in one consent.
- **Every mailbox you own is a way in** to the same account,
  interchangeably. Connecting another mailbox while signed in adds it as a
  sign-in method. Passkeys are a way in too.
- Accounts are linked by **proven ownership**, never by matching email
  addresses.
- **Campbooks issues every token**: the web session, native apps, third-party
  clients and MCP agents.
- The login, consent and passkey screens are **Campbooks screens** in en,
  pt-PT, es-ES and fr.

Rauthy (ADR 0004) links one upstream provider per account, never exposes
the upstream tokens (so sign-in cannot connect a mailbox), and has no API to
set a federation id. ADR 0014 supersedes 0004: Campbooks owns identity end
to end and Rauthy is removed.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/75.

**Modules touched**
- New module **`identity`** (`crates/modules/identity`, schema `identity`).
  It depends on the kernel only, like every module.
- **kernel**: two additions.
  - A local key source for `TokenVerifier`, so the API reads the issuer's
    keys in-process.
  - The `GrantIntake` port (`kernel::connections`) and its `ProviderGrant`
    type, which hand an upstream grant to `connectors`.
- **connectors** (#450): implements `GrantIntake` over
  `create_or_refresh_connection`. Its own Google consent endpoints delegate
  to `identity`.
- **server**: composes `identity`. The BFF's issuer becomes the server
  itself and its client secret is derived (no provisioning). The `identity`
  protocol routes are mounted outside `/api/v1`.
- **clients/app**: new `identity` module (sign-in, new account, consent,
  passkeys, ways to sign in). The first-run UI (#451) gains the "connect
  this mailbox here" step.
- **deploy/docs**: Rauthy's services, `deploy/idp/` and their settings are
  removed.

No cross-module dependency is added. `identity` ↔ `connectors` goes through
a kernel port (the pattern of `Membership`), so `architecture.toml` gets no
entry.

## What Changes

- **Accounts and credentials.** An account has one or more **ways in**:
  provider identities (provider, issuer, subject, the email at link time) and
  passkeys. The token `sub` is the account id. An account can never lose
  its last way in.
- **Provider sign-in with the mailbox in the same consent.** Google first,
  then Microsoft and Zoho. Sign-in asks for identity scopes plus the mail
  scopes `connectors` declares for that provider, with offline access. An
  unknown provider identity creates a new account, after a "new account"
  confirmation. A known one signs in. Signed in, a round trip links the
  identity to the current account, and is refused if another account already
  holds it.
- **The mailbox grant goes to `connectors`.** It is handed over through
  `GrantIntake` into the current workspace. On a first sign-up, before any
  workspace exists, it is held sealed and adopted when the person creates or
  picks their first workspace. If they never do, it is revoked at the
  provider after 7 days.
- **Authorization server**, in-process, on the app origin:
  - discovery (OIDC + RFC 8414) and RFC 9728 resource metadata;
  - `/oauth2/authorize` with code + PKCE S256 only, single-use codes and RFC
    9207 `iss`;
  - `/oauth2/token`: authorization_code, refresh_token (rotation with reuse
    detection), client_credentials;
  - `/oauth2/revoke`, `/oauth2/jwks`, `/oauth2/userinfo`,
    `/oauth2/end_session`;
  - JWT access tokens (RFC 9068) signed with Ed25519, with keys rotated
    automatically.
- **Clients.**
  - First-party clients are created by migration: `campbooks-web` (the BFF)
    and `campbooks-native` (Tauri, public, loopback and private-use-scheme
    redirects).
  - Workspace clients are registered through the API by owners and admins:
    confidential (client_credentials, `workspace_id` claim) or public.
  - Dynamic Client Registration (RFC 7591) and Client ID Metadata Documents
    for MCP agents.
  - A consent screen for every third-party client. Its tokens are bound to
    one workspace the person picks.
- **Passkeys.** Register while signed in, sign in, and sign up with a
  passkey alone (self-hosters with no provider app, IMAP-only users).
- **BFF.** It signs in against the in-process issuer. Logout ends the issuer
  session too, with no confirmation page.
- **Removal of Rauthy.**
  - Gone: the `idp`, `idp-init` and `idp-provision` compose services,
    `deploy/idp/`, `xtask/tests/idp_provision.rs`, the
    `CAMPBOOKS_IDP_*`/`CAMPBOOKS_BFF__IDP_INTERNAL_URL`/`CAMPBOOKS_BFF__CLIENT_SECRET*`
    settings and the Rauthy sections of `deploy/README.md`.
  - ADR 0004 is marked superseded. The `upstream-sign-in` change is
    withdrawn.
  - The relevant `openspec/decisions-needed.md` entries are resolved: the
    Rauthy bump, the IdP origin, the logout page and one provider per
    account.

## Non-goals

- **Passwords**, email one-time codes and password reset.
- **Account merging.** Two accounts are never merged, not even with proof of
  both; a way in can be moved by removing it from one account and linking it
  to the other.
- **Account deletion.** It needs every module (a follow-up issue).
- **"Sign in with Campbooks" for other websites.** ID tokens for third-party
  OpenID relying parties (RS256 ID tokens, pairwise subjects) are out of
  scope; third parties get access tokens to our API.
- **IMAP mailboxes as ways in.** There is no OAuth proof of ownership for
  them (#74).
- DPoP / sender-constrained tokens, token introspection (RFC 7662),
  back-channel logout to third-party clients, SAML, SCIM.
- Campbooks-side second factors: providers keep their own MFA, and passkeys
  verify the user.
- Supporting an outside identity provider as the issuer. The verifier stays
  standard, so this can return later, but nothing is built or tested for it.
- The MCP server itself (milestone M4). This change only makes the authorization side
  ready for it.
- Google's restricted-scope verification (#53) and the security review
  (#57). Both must happen before production.
