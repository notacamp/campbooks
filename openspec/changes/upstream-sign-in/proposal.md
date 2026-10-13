# Proposal

## Why

People want to sign in to Campbooks with the account they already have:
Google, Microsoft or Zoho. Sign-in belongs to the identity provider (Rauthy,
ADR 0004), which supports "upstream" providers, but nothing registers them:
a self-hoster would have to click through Rauthy's admin UI, which needs a
passkey-protected admin session and knowledge of each provider's endpoints.
The owner wants to configure the providers from `.env`, like everything else,
and pick the sign-in method on the login page.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/71.
Modules touched: none. This is deployment tooling only (`deploy/idp`,
`deploy/compose.yaml`, docs) plus a test harness in `xtask`.

## What Changes

- **Providers from settings.** `idp-provision` registers Google, Microsoft
  and Zoho in Rauthy through its admin API when their client ID is set
  (`CAMPBOOKS_IDP_GOOGLE_CLIENT_ID`, `..._MICROSOFT_...`, `..._ZOHO_...`).
  Client secrets come from files (`deploy/secrets/<provider>-client-secret`,
  mounted read-only), never from the environment. A provider whose client ID
  is unset is not registered, and the login page shows passkey and password
  only.
- **Microsoft tenant and Zoho region are settings.**
  `CAMPBOOKS_IDP_MICROSOFT_TENANT` (default `common`: work, school and
  personal accounts) and `CAMPBOOKS_IDP_ZOHO_ISSUER` (default
  `https://accounts.zoho.eu`).
- **Who may create an account through a provider** follows
  `CAMPBOOKS_SIGNUP__POLICY`: `open`, `code` and `approval` let new people in
  (Campbooks then applies its policy to creating a workspace); `invite_only`
  does not (only accounts that already exist may sign in with a provider).
  `CAMPBOOKS_IDP_PROVIDER_SIGNUP` (`auto`, `open`, `closed`) overrides it.
- **Idempotent, admin edits survive.** A provider is created once and marked
  done. It is re-applied only when its settings in `.env` or its secret file
  change (so rotating a secret is "replace the file, `up`"). An existing
  provider with the same issuer is adopted instead of duplicated.
- **The IdP's API key may manage providers** (new installs). Installs from
  before this release grant it once in the admin UI; `idp-provision` says how
  when it is refused.
- **Docs.** `deploy/README.md` "Sign in with Google, Microsoft or Zoho": the
  exact redirect URL and console steps per provider, account linking, and the
  behaviour per signup policy. `.env.example`, CHANGELOG, ADR 0004 findings.
- **Tests.** `xtask/tests/idp_provision.rs` runs `provision.sh` against a fake
  Rauthy admin API (wiremock): skip when unset, create once, idempotency,
  re-apply on change, adoption, policy mapping, missing rights, no secret in
  any log line.

## Non-goals

- Campbooks-side sign-in UI. The login page is Rauthy's; theming it and
  showing provider logos is a follow-up (the API can upload logos).
- Linking several providers to one account. Rauthy links an account to at
  most one upstream provider (see design); recorded in
  `openspec/decisions-needed.md`.
- Connecting a mailbox (campbooks-cloud#36). Signing in is not connecting a
  mailbox, though one consent could later offer both.
- Deep links that skip Rauthy's picker (`idp_hint`) from a Campbooks screen.
- Other providers (GitHub, Apple, generic OIDC) from settings; they can be
  added in the admin UI today.
