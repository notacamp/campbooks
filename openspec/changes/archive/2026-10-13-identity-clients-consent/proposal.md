# Proposal

## Why

ADR 0014 makes Campbooks the authorization server of its own API. Slices
S1a, S1b and S2 built the issuer, the sign-in screens and the ways in, all
for Campbooks' own web app. Nothing else can get a token yet:

- the native client `campbooks-native` was switched off by the S1a security
  review. A signed-in browser would give a code to whatever listened on a
  loopback port, and no one would be asked (RFC 8252 §8.6);
- a workspace can't register an integration of its own, and there is no
  `client_credentials` grant;
- MCP agents (the assistant clients people already use) have no way to
  register, find out which issuer protects the API, or ask a person for
  consent.

This change is **slice S4** (campbooks-cloud#81, parent #75).

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/81

**Modules touched**
- **identity**:
  - stored clients (workspace, dynamically registered, metadata-document);
  - consents and the consent step of `/oauth2/authorize`;
  - `client_credentials`, RFC 8707 resources and RFC 9728 metadata;
  - `/oauth2/register` (RFC 7591) and Client ID Metadata Documents behind an
    SSRF guard;
  - the operations `identity.list_consents`, `identity.revoke_consent`,
    `identity.list_clients`, `identity.register_client`,
    `identity.rotate_client_secret` and `identity.remove_client`;
  - the events `identity.client_registered`, `identity.client_removed`,
    `identity.consent_granted` and `identity.consent_revoked`;
  - consumers of `workspaces.member_left` and
    `workspaces.workspace_deleting`.
- **kernel**:
  - `tenancy::Workspace<S>` checks the `workspace_id` claim on user tokens
    too;
  - the in-process verifier keeps that claim for users, and its audience is
    the API's resource URI;
  - `tenancy::Membership::workspaces_of` lists a person's workspaces for the
    consent screen;
  - `auth::challenge_with_resource_metadata` adds RFC 9728's
    `resource_metadata` to Bearer challenges.
- **workspaces**: implements `Membership::workspaces_of`.
- **server**: mounts the new protocol routes and adds `resource_metadata` to
  the API's 401 and 403 challenges.
- **clients/app**:
  - the consent screen (`/consent`);
  - Settings → Apps and integrations (connected apps; the workspace's own
    clients);
  - a link to it from the account menu.

No cross-module dependency is added. `identity` learns about workspaces
through the kernel's `Membership` port, as `Workspace<S>` already does.

## What Changes

- **Native apps**: `campbooks-native` is on again. Every authorization it
  asks for shows the consent screen, which names the app, what it may do and
  the account, and is never skipped. The screen follows a sign-in from the
  last ten minutes.
- **Consent** for every client that isn't the web app:
  - the screen shows the client, whether it is verified, its redirect host,
    the scopes in plain words, and the workspace it may use;
  - Allow needs a recent sign-in;
  - an Allow is remembered, and later authorizations skip the screen, only
    when the client's redirect is an exact `https` URI. A loopback or
    private-use-scheme redirect is asked for every time;
  - connected apps are listed and revocable, and revoking ends every token
    the app holds.
- **Workspace-bound tokens**: a third-party token carries the workspace the
  person picked (`workspace_id`), and the API answers 404 for any other
  workspace.
- **Workspace clients** (Settings → Apps and integrations, owners and
  admins, `identity:clients`):
  - confidential clients use `client_credentials` and are bound to their
    workspace;
  - public clients use code + PKCE with exact redirect URIs;
  - the secret (256 bits, stored as SHA-256) is shown once and can be
    rotated; creating or rotating it needs a recent sign-in.
- **Scopes for third parties**: `identity:*` (accounts, ways in, clients)
  are granted only to Campbooks' own clients.
- **RFC 8707**: `resource` is checked (`invalid_target`). Every access token
  names the API's resource URI, `<public origin>/api`, as `aud`.
- **RFC 9728**:
  - `/.well-known/oauth-protected-resource` and
    `/.well-known/oauth-protected-resource/api`;
  - the API's 401 and 403 challenges carry `resource_metadata`.
- **RFC 7591** `/oauth2/register`:
  - public clients only, with `https` or loopback redirect URIs;
  - rate-limited per address;
  - deleted after 30 days unused;
  - always shown on consent as unverified.
- **Client ID Metadata Documents**: an `https` `client_id` URL is fetched
  through an SSRF guard (public addresses only, the checked address pinned,
  no redirects, 5 s, 64 KiB) and cached up to 24 hours.
- **Rate limits** (the kernel limiter): `/oauth2/register`, `/oauth2/token`
  and the consent action.
- **Discovery** lists the `client_credentials` grant, the
  `registration_endpoint` and `client_id_metadata_document_supported`.

## Non-goals

- The Tauri side of native sign-in (#17). This slice makes the server ready
  for it and proves it with a scripted loopback client.
- An MCP server endpoint. The resource list holds the API only; adding the
  MCP endpoint later is one more accepted `resource`.
- RFC 7592 (managing a registered client), private-key JWT client
  authentication, and confidential dynamic clients.
- Token introspection, and revoking access tokens before they expire (they
  live ten minutes).
- Client logos from dynamic or metadata-document clients: only a
  workspace's own client may show one, because anyone can register the
  others.
- "Sign in with Campbooks" for other sites (ID tokens to third parties).
  Third parties get access tokens. They may get an ID token for `openid`,
  but RS256 support is not added.
