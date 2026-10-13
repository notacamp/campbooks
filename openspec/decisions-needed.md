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
**Recommended:** decide in the BFF issue (campbooks-cloud#16). Proxying under the app origin
needs Rauthy's `cookie_mode = secure` and care with the CSP; a separate host
is simpler and is what ships now.

## Report Rauthy issues upstream

The IdP spike found that client_credentials rejects HTTP Basic client
authentication although discovery advertises it. (The missing Portuguese and
Spanish translations are contributed upstream in sebadob/rauthy#1799.)
**Recommended:** a maintainer files the Basic-auth bug upstream.
