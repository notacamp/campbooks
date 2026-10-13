# Design

## Context

Campbooks runs with real mail in production. ADR 0005 routes every AI call
through a role; ADR 0012 keeps an event log and a job queue in Postgres. The
audit (campbooks-cloud#117) asked that mail never leave the instance without
an explicit operator decision, that errors and caches never keep it, and that
kernel history be bounded (#65).

## Data ownership and events

- No new tables. `kernel.events` and `kernel.jobs` (schema `kernel`, owned by
  the kernel) gain indexes for pruning (`kernel__retention` migration).
- New kernel schedule `kernel.prune_events` (hourly, queue `maintenance`);
  `kernel.prune_jobs` also prunes failed jobs. No events are published or
  consumed by this change.
- `workspaces.purge_workspace` calls `campbooks_kernel::jobs::purge_workspace`
  (finished jobs and undelivered-safe events of the workspace) next to
  `changes::purge_workspace`, in the same transaction.
- No new endpoint and no new scope. `GET /admin/v1/system/overview`
  (`system.read`) gains two fields per AI role.

## AI use

Roles: all four. When a role is off the instance's network and not opted in,
it is treated as unavailable for content: `chat`/`embed`/`decide` fail with
`ExternalContentRefused` (problem `ai_external_content_refused`, 503) and the
`decide` pass is not composed, so the deterministic rules sort mail as they do
when `decide` is not configured. Nothing is undone because nothing was sent.
`complete` has no caller yet; only the health probe reaches it.

## Decisions

### "On the instance's network" is decided from the URL and configuration

`on_instance_network(url, internal_hosts)`:

- IP literals: loopback, RFC 1918 (`10/8`, `172.16/12`, `192.168/16`), IPv6
  unique-local `fc00::/7` (and IPv4-mapped forms). Link-local (`169.254/16`,
  cloud metadata) and carrier-grade NAT (`100.64/10`) are **not** on the
  network: neither is a place an operator runs a model server by design.
- Names: `localhost`, `*.localhost`, any single-label name (compose services:
  `laya`, `ollama`, `embeddings`), or a name in
  `CAMPBOOKS_AI__INTERNAL_HOSTS` (exact, or `.domain` for a subtree).
- DNS is never consulted: a name's answer can change between the check and the
  request, and a check that depends on the resolver is a guess, not
  configuration.

A single-label name can be made to resolve elsewhere through a search
domain; that is a deliberate operator setup and documented.

### The opt-in is per role and defaults to deny

`CAMPBOOKS_AI__<ROLE>__EXTERNAL_CONTENT=allow|deny` (blank = deny). A role
section with neither `BASE_URL` nor `MODEL` is ignored, so a compose file can
carry the opt-in while the role's other settings are commented out in `.env`.

### The gate lives in the gateway; the module also skips the pass

Every content call resolves its provider through
`AiConfig::provider_for_content`, so no module can forget the check. The
conversations module additionally leaves the model strategy out when the
`decide` role is refused, so no job is enqueued to fail and the operator sees
one warning at startup instead of a failed job per message.

### Redaction by kind, at the source

`Error::Status` drops the body entirely (it is read to free the connection).
`campbooks_kernel::redact::{json, sqlx, error}` turn errors into "what and
where" strings. Call sites that store or log an error use them; the kernel's
`IntoResponse` for `Error::Internal` logs `redact::error(source)` only, never
`Debug`.

### One `no-store` middleware by path prefix

A `from_fn` layer in the server's outer stack sets `Cache-Control: no-store`
(keeping a handler's own `no-store` variant such as `private, no-store`) and
`Pragma: no-cache` for `/api/`, `/bff/` and `/admin/`. The web client's assets
keep their caching.

### Retention

- Events: `CAMPBOOKS_EVENTS__RETENTION_DAYS` (default 90), hourly, in batches
  of 5 000, skipping events a pending or running job still delivers.
- Failed jobs: `CAMPBOOKS_JOBS__FAILED_RETENTION_DAYS` (default 30), in the
  existing `kernel.prune_jobs` with succeeded jobs.
- Purge: finished jobs and events of the workspace; unfinished jobs stay
  (other modules' purges may be queued for the same moment).

### Recordings

`scripts/record-target.ts` refuses a base URL whose host is not
`localhost`/`127.0.0.1`/`[::1]` unless `--allow-remote`; non-http(s) URLs are
refused even with the flag.

## Alternatives considered

### A. Classify endpoints by resolving DNS to private addresses

Accurate for most setups and needs no list, but it is a guess at request
time: DNS can change (rebinding), and a resolver failure would decide
privacy. Rejected for explicit rules plus `CAMPBOOKS_AI__INTERNAL_HOSTS`.

### B. A single global switch (`CAMPBOOKS_AI__EXTERNAL_CONTENT=allow`)

Simpler, but an operator approving one provider (for drafts) would silently
approve every other role. Per-role opt-in (chosen) costs one line per role.

### C. Per-role "location" setting the operator must always set

(`CAMPBOOKS_AI__<ROLE>__LOCATION=instance|external`.) Fully explicit, but it
breaks every existing on-box setup on upgrade and invites marking a hosted
endpoint "instance" to silence the check. The chosen rules need no setting for
the common compose layout and fail closed for everything else.

### D. Keep provider bodies but scrub them

Pattern-scrubbing addresses and subjects from bodies is incomplete by nature.
Dropping the body loses some diagnosis (operators read the provider's own
logs or reproduce with the eval tool). Chosen: drop.

### E. Cache headers per handler

Each handler sets its own headers. Rejected: one forgotten handler leaks; a
path-prefix middleware has no such failure mode.

### F. Event retention by partitioning `kernel.events` by month

Dropping partitions is cheaper than `DELETE`, but it needs partition
management and changes the table's primary key; the event volume of one
instance does not justify it yet. Batched deletes with an index (chosen) keep
the schema as is.

### Copied from the Rails app on `main`?

No. The Rails app had no AI egress gate, logged full exceptions to its error
tracker and kept job history in Solid Queue indefinitely. Nothing here copies
it.

## Risks

- A self-hoster pointing a role at a hosted provider loses that role on
  upgrade until opting in. Called out in the CHANGELOG and shown in Admin →
  System; the server logs a warning at startup for `decide`.
- Replays reach back 90 days at most; a consumer added later cannot replay
  older history. Projections that need more rebuild from module tables.
- The kernel tables get row-level security in batch 1 of #117; the
  maintenance jobs run without a workspace and must keep working under those
  policies (the same is true of `kernel.prune_jobs` today).
