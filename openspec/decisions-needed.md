# Decisions needed

Open questions that need a human. One heading per decision, each with the
options and a recommended answer. Remove an entry once it is decided and
recorded (ADR, spec or issue).

## Decide-role model ("laya")

The brief named a model for the `decide` role that reads as "laya". We assume
it means the **Llama** family; the catalog ships Llama 3.1 8B Instruct as one
`decide` suggestion, alongside Qwen3 8B and Mistral Small. Any model is
configurable regardless.
**Recommended:** confirm the intended model, or let the AI-gateway benchmark
task pick the default by measured accuracy on our classification set.

## Platform direction (custom models + workflows + dashboards)

Whether Campbooks should become a platform (user-defined models, workflow
automation over any model, a dashboard builder) with mail, calendar, tasks,
files and accounting as built-in modules on the same primitives.
**Recommended:** decide as part of the domain-mapping task, before any domain
module beyond `system` is specced; it changes how modules store data.

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
**Recommended:** decide in the BFF issue (campbooks-cloud#16). Proxying under the app origin
needs Rauthy's `cookie_mode = secure` and care with the CSP; a separate host
is simpler and is what ships now.

## Report Rauthy issues upstream

Two upstream findings from the IdP spike: client_credentials rejects HTTP
Basic client authentication although discovery advertises it, and the login
UI has no Portuguese or Spanish translation.
**Recommended:** a maintainer files the Basic-auth bug upstream; we
contribute pt and es translations to Rauthy before the login page is
user-facing.
