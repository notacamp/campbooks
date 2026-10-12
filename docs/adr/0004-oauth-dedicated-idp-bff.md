# 0004 — OAuth 2.1 with a dedicated IdP; BFF for web, keychain for native

**Status:** Accepted · 2026-10-12 — IdP product choice pending a spike

## Context
First-party clients (web, desktop, mobile) and third-party clients
(integrations, scripts, MCP agents) must authenticate with the same standard.
Tokens in browser JavaScript are the weakest link.

## Decision
- **A dedicated OIDC/OAuth 2.1 identity provider runs as its own container**
  beside the API. First candidate: **Rauthy** (Rust, passkeys, Postgres);
  fallback: **Ory Hydra** with our own login/consent screens. A time-boxed
  spike confirms the pick (client_credentials, PKCE, refresh rotation, dynamic
  client registration, custom login branding, upgrade story for self-hosters).
- **The API is a pure resource server:** validates JWT access tokens
  (issuer, audience, expiry, signature via cached JWKS) and checks a scope on
  every handler. Scopes are a catalogue in the kernel, mirrored in the IdP.
- **Web → BFF.** The server runs the OIDC authorization-code flow itself,
  keeps tokens server-side, and gives the browser only an httpOnly,
  `Secure`, `SameSite=Strict` session cookie. `/bff/*` proxies to the API with
  the token attached. CSRF protection on unsafe methods.
- **Native (Tauri) → public client.** Authorization code + PKCE in the system
  browser (loopback or claimed-scheme redirect). Refresh token in the OS
  keychain/keystore, held by the Rust side; webview JS never sees it.
  Consider DPoP for sender-constrained tokens.
- **External clients:** authorization code + PKCE (user-delegated) and
  client_credentials (machine-to-machine), registered per workspace.

## Consequences
- One more container in docker compose; its database lives in the same
  Postgres server under its own database/schema.
- Login, signup and passkey screens are served by (or themed in) the IdP.
- Workspace membership and roles stay in Campbooks, keyed by the IdP subject.

## Alternatives considered
- Auth server embedded in the API — one binary, but we would own and audit
  security-critical token issuance ourselves.
- Tokens in the SPA (localStorage/memory) — rejected on criterion (2).
