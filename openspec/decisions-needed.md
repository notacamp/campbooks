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

## Model endpoints for decide evals (Laya + an open-weight instruct model)

`cargo xtask eval decide` (campbooks-cloud#59) compares the triage strategies
per question, but only the offline ones (email signals, lexical k-NN, their
cascade) have been run: there is no Laya, instruct-model or embedding
endpoint available to the agents, and no local Docker. Laya and the instruct
model are implemented and tested against mocks and report "not run". The ADR
amending 0009 with the strategy per question waits for those numbers.
**Recommended:** a small hosted open-weight endpoint (an instruct model with
logprobs, e.g. one of the catalog's 7–12B models, plus an embedding model),
or the staging host's `ai` compose profile for Laya, reachable by a scheduled
CI job with `CAMPBOOKS_EVAL_*` secrets. Run
`cargo xtask eval decide` once there, then write the ADR from the report.

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

## One sign-in provider per account

Rauthy (0.36 and 0.37) links an account to at most one upstream provider. A
person who signed up with Google cannot also sign in with Zoho on the same
account; the second provider is refused ("already exists but is not linked")
until they unlink Google from their account page. Options: (a) accept it:
one provider plus a password or passkey per account, documented in
`deploy/README.md`; (b) contribute multi-provider linking to Rauthy upstream
(a schema change on their side); (c) let Campbooks merge two IdP subjects with
the same verified email into one Campbooks user (a second source of truth for
identity, against ADR 0004).
**Recommended:** (a) now; open an upstream feature request for (b) and revisit
if people ask for it.

## Microsoft sign-ins start with an unverified email

Microsoft Entra sends no `email_verified` claim, so Rauthy creates accounts
signed up through Microsoft with an unverified email, and accepting a
Campbooks invitation requires a verified one. Options: (a) admins mark the
email verified in the IdP admin UI (documented); (b) contribute to Rauthy a
per-provider "trust this provider's emails" switch or support for Microsoft's
`xms_edov` claim; (c) relax the invitation check for Microsoft accounts in
Campbooks (weakens the guarantee that only the invited address joins).
**Recommended:** (a) now, (b) upstream; never (c).

## Report Rauthy issues upstream

The IdP spike found that client_credentials rejects HTTP Basic client
authentication although discovery advertises it. (The missing Portuguese and
Spanish translations are contributed upstream in sebadob/rauthy#1799.)
**Recommended:** a maintainer files the Basic-auth bug upstream.

## Labels made in Campbooks: create them in Gmail?

Write-back (campbooks-cloud#66) mirrors Gmail labels a member adds or takes
off, but a label created in Campbooks has no Gmail label yet, and the
`changes_requested` intent carries only the label's id (its name is personal
data). Today such a change is recorded as `skipped`
(`label_not_at_provider`). Options: (a) keep Campbooks labels in Campbooks;
(b) create the Gmail label on first use, with conversations adding an
optional label-name blob to `add_label` (an additive change to the v1
contract) and the connector calling `labels.create` (5 units), after which
conversations links the two by name as it already does; (c) ask the member
the first time. **Recommended:** (b), with a per-inbox switch defaulting to
on, since members expect a label they add to show on their phone.
