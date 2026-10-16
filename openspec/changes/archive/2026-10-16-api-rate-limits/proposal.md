# Proposal

## Why

Today only the endpoints anyone can call are rate-limited: sign-in, the token
endpoint, client registration, the BFF's login and search, all per client
address. Nothing caps the authenticated API itself. A stolen token, a buggy
third-party app or a runaway automation rule can read and write as fast as
the server answers, and can send mail until Gmail's own quota stops it, in the
person's name. Tracking issue:
https://github.com/notacamp/campbooks-cloud/issues/151 (owner-approved
2026-10-16).

Modules touched: the kernel (`rate_limit`, `operations`, `config`,
`telemetry`), the server (wiring the admin router, the OpenAPI
descriptions), `conversations` (one line: `send_draft` declares the send
class) and `xtask` (the architecture check learns the new key). No new
module, no cross-module dependency, no migration.

## What Changes

- **One limit at one choke point.** A kernel middleware sits on every route
  `operations::compose` serves, so every bearer-authenticated operation is
  counted whether it is called over HTTP, through the web BFF or in-process
  with `operations::invoke`. The server puts the same middleware on the
  admin router. Public operations (`security(())`) are not counted here.
- **Keyed by principal, not address.** A person is counted per app
  (`sub` + the token's `azp`), a `client_credentials` client per client id.
  Sends are counted per person across all their apps.
- **Four classes**, picked from the operation catalogue (`x-campbooks`), not
  from paths: **read** (`mutates: false`), **write** (`mutates: true`),
  **send** (an operation declaring `rate_limit: send`; today
  `conversations.send_draft`) and **admin** (the admin API). Defaults: 600
  reads and 120 writes a minute, 30 sends per 10 minutes, 120 admin calls a
  minute.
- **In-process callers get their own buckets.** Automation and the
  assistant acting for a person are counted per (person, caller) at the same
  limits, so a looping rule stops without locking the person out of their
  own app; their sends share the person's send budget.
- **Storage: hybrid.** Reads, writes and admin calls are counted in memory
  per process (a token bucket, bounded and swept); sends in Postgres
  (`kernel.rate_limits`, as today's limits), shared by every process.
- **Configurable**: `CAMPBOOKS_RATE_LIMITS__*` (`ENABLED`, and per class a
  count and a window in seconds; `0` turns a class off).
- **Same 429.** Over a limit the answer is today's `rate_limited`
  problem+json with `Retry-After`, in the caller's language. One log line per
  principal when it starts being limited (no token, subject or content) and
  a `campbooks.rate_limit.refusals` counter by class and caller kind.
- **Operation metadata**: `x-campbooks` gains the optional `rate_limit`
  key (`"send"` only, on mutating operations); `cargo xtask arch` checks it.

## Non-goals

- **Unauthenticated abuse of authenticated routes.** A request whose token
  does not verify is not counted (it gets 401 after one signature check).
  Per-address limits for the authenticated API are not added.
- **Exact limits across processes for reads and writes.** With N API
  processes a principal can get up to N times the read/write budget
  (documented). The send class is exact.
- **Per-workspace or per-plan quotas, billing, AI usage caps** (#152), and
  per-operation numbers other than the send override.
- **A 429 entry in every operation's `responses`.** The behaviour is
  described once, in the API's and the admin API's descriptions; only
  `conversations.send_draft` lists it.
- **Invitations as sends.** `workspaces.invite_member` emails nobody yet;
  when the notifications module emails invitations it should declare the
  send class.
- **Client changes.** The web client already handles a 429 without looping
  (one query retry, mutations never retried, the change stream backs off).
