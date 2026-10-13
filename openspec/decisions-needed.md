# Decisions needed

Open questions that need a human. One heading per decision, each with the
options and a recommended answer. Remove an entry once it is decided and
recorded (ADR, spec or issue).

## Default `complete` model (drafting, summaries, chat)

The `decide` role is settled (Laya, ADR 0009). The default open-weight model
for `complete` — drafting replies, summarising, the assistant's conversation,
schema-guided extraction — is not. Candidates in the catalog today: Qwen3 8B,
Mistral Small 3.2 24B, Gemma 3 12B, gpt-oss 20B, Llama 3.1 8B. Any of them is
configurable per instance and per workspace regardless of the default.
**Recommended:** let the AI-gateway eval set (drafting quality in en/pt/es/fr,
extraction accuracy, latency and memory on the reference hardware) pick the
default, and decide the hosted-cloud model together with the GPU sizing issue.

## Rauthy version bump to 0.37.x

The compose file pins Rauthy 0.36.2, the newest release at least two weeks
old. 0.37.1 (2026-10-07) adds RFC 8252 any-port loopback redirects for our
native client, RFC 9068 `at+jwt` access tokens and a hardening batch (one
low-impact open redirect in password resets fixed). A rehearsal upgraded a
0.36.2 volume to 0.37.1 with no manual step and the shipped clients kept working.
**Recommended:** bump to 0.37.1 on or after 2026-10-21, before the Tauri
sign-in (campbooks-cloud#17) lands; the resource server must accept both `JWT` and `at+jwt`.

## Identity provider origin: own host or under the app's URL

The IdP is served from its own origin (`CAMPBOOKS_IDP_URL`, default port 8081).
All of Rauthy's routes live under `/auth/v1/`, so the server could proxy
`/auth/*` to it and self-hosters would expose a single URL and certificate.
The web BFF (campbooks-cloud#16) works either way: browsers are sent to the
issuer's public URL and the server talks to it on the internal one
(`CAMPBOOKS_BFF__IDP_INTERNAL_URL`).
**Recommended:** keep the separate host for now (what ships); revisit with the
sign-in UI, since proxying needs Rauthy's `cookie_mode = secure`, care with
both CSPs, and a path-aware `__Host-` cookie story.

## Web logout with Rauthy: confirmation page, no return to the app

`POST /bff/logout` ends the web session, revokes the refresh token and returns
the IdP's end-session URL with `client_id` and `post_logout_redirect_uri` but
no `id_token_hint`, because that would hand the ID token to browser
JavaScript. Rauthy 0.36 then shows "Do you really want to logout?" and, after
confirming, stays on its own page instead of returning to the app (it only
honours `post_logout_redirect_uri` with an `id_token_hint`). Options:
(a) keep it (standard, slightly clumsy); (b) have the server end the IdP
session itself: a server-to-server `POST` to Rauthy's end-session endpoint with
the stored `id_token_hint` (Rauthy treats requests without fetch metadata as
back-channel and needs no confirmation), then send the browser home; Rauthy-
specific, and it signs the user out of every app on that IdP session; (c) a
one-time same-origin `/bff/logout/continue` that 303s to the IdP with the hint
(the token passes through the URL bar, not JavaScript).
**Recommended:** (b), with (a) as the fallback for other IdPs, decided with
the sign-in UI.

## Report Rauthy issues upstream## Report Rauthy issues upstream

The IdP spike found that client_credentials rejects HTTP Basic client
authentication although discovery advertises it. (The missing Portuguese and
Spanish translations are contributed upstream in sebadob/rauthy#1799.)
**Recommended:** a maintainer files the Basic-auth bug upstream.
