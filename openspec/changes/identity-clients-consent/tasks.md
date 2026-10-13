# Tasks

## 1. Kernel and workspaces

- [x] 1.1 `kernel::rate_limit` (merged on `next` with S2)
- [ ] 1.2 Verifier: keep `workspace_id` on user tokens in in-process (strict) mode; `TokenVerifier::in_process` takes the audience; tests
- [ ] 1.3 `Workspace<S>`: 404 when any token's `workspace_id` names another workspace; tests
- [ ] 1.4 `Membership::workspaces_of` (default: none); `workspaces` implements it; test
- [ ] 1.5 `auth::challenge_with_resource_metadata` layer; test

## 2. Identity: clients and consent

- [ ] 2.1 Migration `identity__clients`: `clients` (forced RLS + authorization-server policy), `consents`, resource and bound-workspace columns; grants
- [ ] 2.2 `Clients::find` over code and table; `campbooks-native` active; redirect rules per kind (exact, loopback any port); third-party scope ceiling (no `identity:*`)
- [ ] 2.3 Authorize: `resource` (`invalid_target`), the consent decision (remembered only for `https` redirects), recent-sign-in redirect, `consent_due` requests
- [ ] 2.4 `GET`/`POST /auth/consent/{request}` (session, CSRF, recent sign-in, membership, workspace client in its workspace, rate limit); deny → `access_denied`
- [ ] 2.5 Tokens carry `aud` = resource and `workspace_id`; refresh keeps them; `client_credentials`; token endpoint rate limit
- [ ] 2.6 `identity.list_consents`, `identity.revoke_consent` (revokes families); `identity.list_clients`, `register_client`, `rotate_client_secret`, `remove_client` (owner/admin, step-up for secrets)
- [ ] 2.7 Events and consumers (`member_left`, `workspace_deleting`); prune of unused dynamic clients
- [ ] 2.8 Discovery: grant types, `registration_endpoint`, `client_id_metadata_document_supported`; RFC 9728 documents

## 3. Identity: MCP

- [ ] 3.1 `POST /oauth2/register` (RFC 7591) with its rate limit
- [ ] 3.2 SSRF-guarded fetcher; Client ID Metadata Documents with cache
- [ ] 3.3 End-to-end tests: native consent, third-party consent remembered or not, workspace binding, client_credentials, DCR + consent + API call, metadata document, every refusal

## 4. Server and clients

- [ ] 4.1 Server: routes, resource-metadata challenge, verifier audience; tests
- [ ] 4.2 `/consent` screen (stories for every state, tests)
- [ ] 4.3 Settings → Apps and integrations (stories, tests); account-menu link
- [ ] 4.4 en / pt-PT / es-ES / fr

## 5. Docs and gates

- [ ] 5.1 `deploy/README.md` (native, clients, MCP, trusted proxies), `docs/architecture.md`, CHANGELOG
- [ ] 5.2 OpenAPI regenerated, events regenerated, client types regenerated, gates green
- [ ] 5.3 UI verified at 375px and desktop; recordings (browser + vhs)
