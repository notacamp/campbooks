# Design

## Context

Rauthy 0.36.2 (pinned) supports upstream authentication providers ("beta" in
its docs). What follows was read in its source at the `v0.36.2` and `v0.37.1`
tags (`src/api/src/auth_providers.rs`,
`src/data/src/entity/auth_providers.rs`, `src/data/src/rauthy_config.rs`) and
its book (`auth_providers/`, `config/user_reg.md`), and the providers'
discovery documents.

- **Admin API** (API key group `AuthProviders`): `POST /auth/v1/providers`
  lists (a POST, and every entry carries its client secret in clear text),
  `POST /auth/v1/providers/create` creates (Rauthy picks a random id),
  `PUT /auth/v1/providers/{id}` replaces the whole configuration,
  `PUT /auth/v1/providers/{id}/img` uploads a logo. The body is
  `ProviderRequest`: `name`, `typ` (`google`, `github`, `oidc`, `custom`),
  `enabled`, `issuer`, the authorization, token, userinfo and JWKS endpoints,
  `client_id`, `client_secret`, `scope`, `use_pkce`, `client_secret_basic`,
  `client_secret_post`, `auto_onboarding`, `auto_link`, and optional
  admin/MFA claim mappings.
- **Redirect URI** a provider must allow:
  `${CAMPBOOKS_IDP_URL}/auth/v1/providers/callback` (https whenever the IdP is
  behind a TLS proxy).
- **Login page:** every enabled provider is a button with its `name` and its
  uploaded logo (none by default; the button then shows the name only).
  `?idp_hint=<provider id>` on the authorize URL skips the picker.
- **Who gets in.** A login is matched on the provider's subject first. If no
  account is linked, Rauthy looks up the email: an existing unlinked account
  is linked only when the provider has `auto_link` (or the user started the
  link from their account page), otherwise the login is refused ("already
  exists but is not linked"). With no account at all, one is created only when
  the provider has `auto_onboarding`; its `email_verified` is the provider's
  claim (false when absent). `[user_registration]` (open self-registration by
  email) is independent and stays off.
- **One provider per account.** A user carries one `auth_provider_id`; an
  account linked to Google cannot also sign in with Zoho, and auto-link never
  applies to an already-linked account. Password and passkey keep working
  beside the link.
- **`auto_link` ignores `email_verified`**, and the upstream ID token is
  decoded without checking its signature (it comes straight from the token
  endpoint over TLS, which OIDC allows).
- **0.37.x** keeps the same endpoints and fields. Changes that matter here:
  the upstream email is lowercased before lookup; a matching email becomes
  verified when the provider says so; the callback is single-use; RFC 8707
  `resource` survives a provider round trip; provider names accept any
  letters plus `()._-`. Nothing in this change needs adjusting for the bump.

Providers, from their discovery documents: Google (issuer
`https://accounts.google.com`, `email_verified` always sent), Microsoft Entra
v2 (`https://login.microsoftonline.com/<tenant>/v2.0`, no `email_verified`;
new multi-tenant apps drop the `email` claim when the domain owner is not
verified), Zoho (one accounts server per data centre, e.g.
`https://accounts.zoho.eu`, `email_verified` sent; a user from another data
centre must exchange the code at their own accounts server, which Rauthy's
fixed token endpoint cannot do).

## Data ownership, events and scopes

No module, schema, table, event, endpoint or scope changes. The providers
live in Rauthy's database (`idp-data` volume). `idp-provision` keeps one
marker per provider in `idp-data` (`campbooks/provisioned/provider-<key>`:
Rauthy's id and a SHA-256 fingerprint of the settings it applied). Client
secrets are read from `deploy/secrets/` (bind mount, read-only) and sent to
Rauthy, which stores them encrypted.

## Decisions

### Fixed endpoints, not Rauthy's discovery lookup
The script writes each provider's endpoints itself (Microsoft and Zoho
derived from the tenant and the accounts server). `scope` is
`openid email profile`, PKCE (S256) on, the secret sent in the body
(`client_secret_post`), which all three accept. Google is Rauthy's built-in
type `google`; Microsoft and Zoho are `oidc`.

### Created once, re-applied only when its settings change
Follows the client marker pattern (admin edits survive restarts and
upgrades), with one addition: the marker holds a fingerprint of the
operator's inputs (client ID, secret, issuer, whether new accounts are
allowed). When those change, the next `up` replaces the provider's
configuration with Campbooks' and says so. The fingerprint covers inputs, not
the script's defaults, so a Campbooks upgrade never rewrites a provider by
itself. Without a marker, a provider with the same issuer is adopted
(configured, then marked) rather than duplicated. A provider whose client ID
is removed from `.env` stays registered; the log says to disable or delete it
in the admin UI, because deleting it would strand the accounts linked to it.

### New accounts follow the signup policy; never auto-link
`auto_onboarding` is on for `open`, `code` and `approval`: those policies let
strangers ask for a workspace, so they must be able to sign in; Campbooks
applies the policy when they try to create one. It is off for `invite_only`,
where only existing accounts may use a provider (an invitee's account is
created by an admin, as today, and linked from the account page; or the
operator sets `CAMPBOOKS_IDP_PROVIDER_SIGNUP=open`). `auto_link` is off for
every provider: Rauthy does not check `email_verified` before linking, and
Microsoft's email claim is not proof of ownership, so auto-link would be an
account-takeover path. Linking an existing account is explicit, from the
account page, after signing in with the existing method.

### Microsoft: `common` by default
Campbooks is for young professionals and small-business owners, many of whom
use personal Outlook accounts; `common` takes work, school and personal
accounts. `organizations`, `consumers` or a tenant ID narrow it. Microsoft
sends no `email_verified`, so a Microsoft-created account is unverified and
cannot accept a Campbooks invitation (which requires a verified email) until
an admin marks it verified; recorded in `openspec/decisions-needed.md`.

### The API key's new right
New installs bootstrap the `campbooks` API key with `AuthProviders` read,
create and update (no delete: the script never deletes). Rauthy applies
bootstrap files only to an empty database and an API key cannot widen its own
rights, so older installs grant it once in the admin UI; the script makes no
provider call when no provider is configured, so they keep working untouched.

### Secrets never in logs or argv
Request bodies carrying a secret go through a 0600 temp file
(`curl --data-binary @file`), never the command line. Provider responses
(which echo the secret) are never printed; an error body that contains the
secret is withheld.

## Alternatives considered

1. **Rauthy's discovery lookup** (`POST /providers/lookup`) to fill the
   endpoints. Follows provider changes automatically, but needs JSON parsing
   in a busybox shell, Microsoft's discovered issuer
   (`.../{tenantid}/v2.0`) fails Rauthy's own validation, and the result would
   depend on the network at provision time. Rejected; the endpoints are stable
   public OIDC URLs and tested.
2. **Strict marker pattern (never touch after creation).** Simplest and what
   clients do, but rotating a secret (Microsoft's expire within two years) or
   changing the signup policy would leave Rauthy silently stale, and the policy
   drift is a security drift. Rejected for the fingerprint.
3. **Always reconcile to `.env` on every `up`.** One source of truth, but
   every admin UI edit (logo aside) would be undone on the next restart.
   Rejected.
4. **`auto_onboarding` always on, Campbooks' policy as the only gate.** One
   gate instead of two and invitees onboard themselves, but under
   `invite_only` any stranger gets an IdP account, a Campbooks session, and
   could become the instance's first user on a fresh install. Kept as the
   explicit opt-in `CAMPBOOKS_IDP_PROVIDER_SIGNUP=open`.
5. **Secrets as environment variables or compose `secrets:`.** Env leaks into
   `docker inspect` and process listings (the project rule is files). Compose
   file secrets fail `up` when the file is missing, which would break every
   install without providers. Rejected for a bind-mounted `secrets/` directory
   shipped empty.
6. **Sign-in in Campbooks itself** (the old Rails app on `main` ran its own
   Google/Zoho/Microsoft OAuth and linked accounts by verified email). Not
   copied: ADR 0004 puts sign-in in the IdP, which already speaks to the
   providers; Campbooks only sees Rauthy's tokens.

## Risks and trade-offs

- Rauthy's upstream support is labelled beta and has one main maintainer
  (ADR 0004). The script only uses its documented admin API.
- Not exercised against the real providers here (needs real OAuth apps); the
  staging rollout is the end-to-end check.
- Zoho users from another data centre cannot sign in through a provider
  configured for this one.
- One provider per account (Rauthy) is a product limitation; see
  `openspec/decisions-needed.md`.
