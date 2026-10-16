# Design

## Context

What was checked on `next` before deciding:

- **`kernel::rate_limit`** counts fixed windows in Postgres
  (`kernel.rate_limits`, one row per HMAC'd bucket, an `INSERT … ON CONFLICT
  DO UPDATE … RETURNING` per hit) and answers `Error::RateLimited` (429
  `rate_limited` + `Retry-After`, localized in all four languages). It is
  used per address by identity's public endpoints and the BFF login, and per
  principal by `search.query` (600/min).
- **Every module route is served through `operations::compose`**: it parses
  the OpenAPI document into the `Catalogue` and wraps the routes in the
  idempotency layer (between two `localize` layers). HTTP requests, the web
  BFF's proxied calls (`/bff/api/*` runs the request through that same
  composed router with the session's access token) and `operations::invoke`
  (automation and the assistant, with the principal attached in-process) all
  pass there. The admin API (`/admin/v1/*`, `/bff/admin/*`) is a separate
  router behind the audit layer; `Admin<P>` verifies the token itself.
- **Token verification** (`TokenVerifier::verify`) is a signature check
  plus, for the in-process issuer's people, one primary-key read
  (revocation, #497). The idempotency layer verifies once and attaches the
  principal (`AuthenticatedPrincipal`) so `Scoped` does not verify again.
- **Operation metadata** (`x-campbooks`, `kernel::Operation`) says whether
  an operation mutates; `query_in: body` marks reads served by POST. The
  architecture check rejects unknown keys.
- **Sending mail**: the API operation is `conversations.send_draft`
  (`conversations:send`); after the undo window a job hands the send to
  `connectors` (an event and a job, no HTTP operation).
- **The web client** (React Query) retries a failed query once, never
  retries mutations, and the change stream reconnects with exponential
  backoff; `staleTime` is 30 s and live updates replace polling.
- **Precedent for per-process caps**: the change feed already limits open
  streams per principal *per process* (`CAMPBOOKS_CHANGES__MAX_STREAMS_PER_PRINCIPAL`).
- **Cost of a Postgres counter** (measured on the dev Postgres 17, Docker on
  a laptop, `pgbench`, 8 clients, 10 s): `SELECT 1` 1.1 ms / 7 400 tps; the
  limiter's upsert over 2 000 buckets 3.1 ms / 2 560 tps; one hot bucket
  (a single principal's concurrent requests, serialized on its row lock)
  4.1 ms / 1 950 tps. Each hit is a committed write (WAL flush) and holds a
  pooled connection (10 by default) for the round trip. An in-memory bucket
  is a hash lookup under a mutex: well under a microsecond.

## Decisions

### Choke point: a middleware on the composed routes

`kernel::rate_limit::api::middleware`, installed by `operations::compose`
between the idempotency layer and the outer `localize` layer, and by the
server on the admin router (before the audit layer). It finds the operation
in the catalogue by `MatchedPath` (as idempotency does), skips public
operations (no scopes), and resolves the principal:

- attached in-process by `invoke` → that principal, and the caller kind
  (`automation` / `assistant`) that `invoke` also attaches;
- otherwise the bearer token, verified once; on success the principal is
  attached for the idempotency layer and `Scoped` to reuse, so the number of
  verifications per request does not change. A token that does not verify
  is not counted: the handler answers 401 as before.

It sits outside the idempotency layer, so a replay counts as a hit and a 429
never claims an idempotency key (the spec already says rate-limit failures
release it). It sits inside the outer `localize`, so the 429 is in the
caller's language on every path (HTTP, BFF, invoke).

### Who is counted: principal, per app

| caller | read / write / admin bucket | send bucket |
|---|---|---|
| a person through an app (web BFF, desktop, a third-party app) | person + app (`sub`, `azp`) | person (`sub`) |
| a `client_credentials` client | client id | client id |
| automation for a person (`invoke`) | person + `automation` | person |
| the assistant for a person (`invoke`) | person + `assistant` | person |

**A person using several apps** gets a read/write budget per app. A
misbehaving third-party app the person allowed then exhausts only its own
budget and never locks the person out of the web app they would use to
revoke it; the total stays bounded by the handful of apps a person consents
to. **Sends** are counted per person across every app and in-process caller:
the mail leaves in the person's name whoever pressed send, and what the cap
protects (recipients, the mailbox's provider quota and reputation) is the
person's, not the app's.

**In-process principals are limited, in buckets of their own**, at the same
class limits:

- *Exempt* was rejected: a rule whose action triggers itself, or a model
  stuck in a tool loop, would run unbounded.
- *Sharing the person's bucket* was rejected: a looping rule would lock the
  person out of the app they need to turn it off, and an assistant run would
  make the person's next click fail.
- Own buckets per (person, caller) stop a loop at the same ceiling a script
  hits, while the person keeps working. Automation runs in the worker
  process, so its buckets live there.

A stolen token shares the bucket of the app it was issued to (the person's
web app for a stolen web session): an abuse cap, not isolation.

### Classes from the catalogue, with one declared override

- **read**: `mutates: false` (including `query_in: body` reads served by
  POST);
- **write**: `mutates: true` (or, with no metadata, any method but
  GET/HEAD/OPTIONS/TRACE, as idempotency decides);
- **send**: an operation whose `x-campbooks` says `rate_limit: "send"`
  (`Operation::command(..).rate_limit(RateLimit::Send)`); the override
  replaces the write class. Only `send` exists and only on mutating
  operations (the architecture check enforces both), so a declaration can
  only make a limit tighter, never loosen one. Today:
  `conversations.send_draft`. `cancel_send` stays a write: taking a send back
  must never be refused for having sent too much.
- **admin**: every operation of the admin router (low volume, audited,
  admin-only; still capped against a stolen admin token). The admin
  middleware counts before `Admin<P>` checks the profile, so a non-admin
  hammering the admin API also stops costing a profile lookup per request.
  It answers 429 the same way to admins and non-admins, so it tells nobody
  who is an administrator; the admin routes are public in `openapi/admin.json`
  anyway.

Classifying by HTTP method alone was rejected: it would count search's POST
reads as writes. Classifying by path was rejected: the catalogue already
says what each operation is, and a new operation is classified with no extra
step.

### Defaults

| class | default | window | reasoning |
|---|---|---|---|
| read | 600 | 60 s | Burst of 600 then 10/s sustained. A page load is 10–20 queries; a fast keyboard session is at most ~3 requests per action at ~3 actions a second, for seconds, not a minute. Live updates replace polling. Search already allows 600/min. A scraper is held to 10/s. |
| write | 120 | 60 s | 2/s sustained, burst of 120: a minute of frenzied keyboard triage. The web client has no per-item bulk loops. |
| send | 30 | 600 s | A person rarely sends more than a handful of mails in ten minutes; 30 covers an inbox-zero reply session. Caps a stolen token at ~180 mails an hour instead of the provider's whole daily quota. |
| admin | 120 | 60 s | An admin screen is a few requests; 2/s sustained is far above clicking through screens. |

The reads/writes are a token bucket (GCRA): `max` hits may arrive at once,
then one every `window / max`; `Retry-After` is the time until the next one
is allowed (at least 1 s). Refused hits are not counted, so `Retry-After` is
exact; hammering does not help since the rate is fixed. Sends reuse the
existing fixed window (a refused hit still counts there, as today).

### Storage: hybrid (option c)

Reads, writes and admin calls in memory per process; sends in Postgres.

### Configuration

`CAMPBOOKS_RATE_LIMITS__ENABLED` (default `true`; `false` turns every
per-principal limit off) and, per class, a count and a window:
`READS`/`READS_WINDOW_SECONDS`, `WRITES`/`WRITES_WINDOW_SECONDS`,
`SENDS`/`SENDS_WINDOW_SECONDS`, `ADMIN`/`ADMIN_WINDOW_SECONDS`. A count of
`0` turns that class off; a window must be 1 s to 1 day (startup refuses
anything else, naming the setting). Flat names in the existing style
(`…_SECONDS`), rather than nested `READS__MAX`, so setting one value never
silently resets another's per-class default. The public-endpoint limits
(sign-in, token, registration, search) are unchanged and not affected by
these settings.

### Memory bound

Buckets live in a `HashMap<u64, Bucket>` keyed by a keyed hash (per-process
random SipHash) of (class, principal, app or caller): no subject or client id
is kept in memory. A bucket whose theoretical arrival time has passed is
full, so dropping it loses nothing: a sweep removes those at most once a
minute. A hard cap (100 000 buckets, about 4 MB) is enforced on insert: the
sweep runs, and if that is not enough the buckets closest to full are
dropped down to 90 % of the cap (only reachable by an attacker holding tens
of thousands of valid tokens).

### Telemetry

The first refusal of a bucket after it last let a hit through logs one
`info` line (class, principal kind, caller kind, operation id, retry-after;
never a token, subject, client id or content), so a hammering script cannot
flood the logs. Every refusal increments `campbooks.rate_limit.refusals`
(attributes: class, principal kind, caller kind). The HTTP metrics already
record the 429 per route with `error.type = rate_limited`.

### Data ownership, events, scopes

- No new table, column or migration: sends use `kernel.rate_limits`
  (schema `kernel`), with buckets HMAC'd under the instance secret as today.
- No event published or consumed, no new scope, no new endpoint. The
  middleware applies whatever scope each operation already requires; it
  never grants anything.
- No AI use.

### OpenAPI

`x-campbooks.rate_limit` (`"send"`) appears on `conversations.send_draft`,
which also lists its 429 response (the one limit a person might meet in normal
use). The API's and the admin API's descriptions say that bearer operations
are rate-limited per caller with 429 `rate_limited` + `Retry-After`. A 429 in
every other operation's `responses` is not added (a non-goal; the clients
branch on the problem `code`).

### Verifications per request

Unchanged on the API: the middleware verifies the token once and attaches
the principal, which the idempotency layer and `Scoped` reuse instead of
verifying again. On the admin router `Admin<P>` keeps verifying the token
itself (it never accepts an in-process principal, by design), so an admin
call verifies twice; admin traffic is small.

### The web client

Checked, not changed: React Query retries a failed query once and then shows
its error state, mutations are never retried, and the change stream
reconnects with exponential backoff, so a 429 neither crashes nor loops.
Honouring `Retry-After` in the query retry is a possible refinement.

## Alternatives considered

### Storage

- **(a) Postgres counters for everything.** Exact across processes, no new
  machinery (the existing limiter). But every API request becomes a write
  transaction: by the measurement above 2–3 ms more per request at load, a
  WAL flush and a pooled connection held for it, concurrent requests of one
  principal serialized on one row lock, and a hot table of dead tuples for
  autovacuum. On the self-host default (one process, a 10-connection pool)
  that is a large cost to make *reads* exact, which nobody needs from an
  abuse cap. Rejected.
- **(b) In-process token buckets for everything.** Cheapest, no database
  traffic, no new dependency. The effective limit scales with the number of
  API processes (fine for coarse caps; the self-host default is one process).
  But the send cap would also scale, and a send is the one class where the
  number matters (recipients, provider quota). Rejected for sends only.
- **(c) Hybrid: in-process for reads, writes and admin; Postgres for
  sends.** Reads and writes cost nothing measurable; sends are exact across
  every process and rare enough (≤ 30 per 10 min per person) that a write
  per send is noise next to the send itself (a provider call and a job).
  Two mechanisms, but the Postgres one already exists. **Chosen.**
- **(d) A shared external store (Redis/Valkey) or a cache in front of
  Postgres.** Exact and cheap per hit, but a new service every self-hoster
  must run (ADR 0006: one image plus Postgres). Rejected. Also considered
  **batching** in-process counts and flushing them to Postgres every few
  seconds (exact-ish across processes at a fraction of the writes): more
  machinery and a sync loop for precision reads and writes do not need;
  noted as the upgrade path if the cloud ever runs many API replicas and
  wants tighter numbers.
- **The `governor` crate** (keyed GCRA with eviction) instead of ~100 lines
  of our own: a new dependency tree (`dashmap`, `quanta`, …) for a small
  algorithm, its own clock abstraction next to `kernel::clock`, and keys
  stored as-is. Rejected; the hand-written bucket is unit-tested against our
  clock seam.

### Choke point

- **Inside `Scoped<S>` / `Admin<P>`.** Runs exactly where the principal is
  known, but an extractor does not know the operation (no class without the
  catalogue), a handler with two extractors would count twice, `Workspace`
  and `Unbound` wrap it, and in-process calls are indistinguishable from the
  idempotency layer's attached principal. Rejected.
- **In `TokenVerifier::verify`.** One place for every token, but it knows
  neither the operation nor the caller kind, runs several times on some
  paths (locale, idempotency) and not at all for in-process principals.
  Rejected.
- **An outer server-wide layer** (next to `no_store`). Would miss `invoke`
  (which reaches the composed router directly) and would need its own route
  matching. Rejected in favour of `compose`, which every path goes through.

### Does it copy the Rails app on `main`?

No. The Rails app throttles its public REST API only, with one class
(600/min per OAuth application, falling back to the IP) counted in
`Rails.cache` (Solid Cache, a database write per request), and never limits
the web UI's session traffic. Here every path is limited (BFF and in-process
included), per person and app, with read/write/send classes from the
operation catalogue and in-process counting for the hot classes. Only the
600/min read figure coincides, and it is argued on its own above.

## Risks

- **Several API processes** multiply the read/write/admin budget by the
  process count (documented in `deploy/README.md`). Acceptable for abuse
  caps; option (d)'s batching is the upgrade path.
- **A restart** resets in-memory buckets: a script gets one fresh burst per
  restart. Sends are unaffected.
- **Defaults too low for someone**: every class is configurable and can be
  turned off; the refusal log line and metric show who is hitting what.
