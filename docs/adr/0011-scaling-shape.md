# 0011 — One binary in three roles; projections from an event log; blobs outside Postgres

**Status:** Proposed · 2026-10-12

## Context
The same code must run as a single container for a self-hoster and scale out
for the hosted cloud. The heaviest work (provider sync, the AI pass, file
processing) has a different profile from serving the API. Cross-cutting
screens (what needs me, search, a person's standing) read from many modules,
and message bodies and files are large.

## Decision
- **One binary, three roles.** `campbooks-server` runs as `api` (HTTP, BFF,
  change feed), `worker` (event consumers, the AI pass, projections, rules)
  and `sync` (connector scheduling and adapters). `campbooks-server all` runs
  every role in one process — the self-host default. The cloud runs each role
  as its own replicas, scaled on its own signal (requests, queue depth, sync
  backlog).
- **Durable queue and outbox in Postgres** (issue #19): domain events are
  written in the same transaction as the change; consumers are idempotent;
  work is partitioned by workspace for fairness. No extra broker to operate.
- **Read models are projections.** Attention, search, standings and the
  client change feed are built by consuming the event log, are disposable, and
  are rebuilt by replay when their logic changes. No timed refresh jobs.
- **Blobs outside Postgres.** Message bodies (raw MIME), attachments and files
  live in object storage (local disk for self-hosters, any S3-compatible
  store otherwise), content-addressed per workspace so one attachment sent to
  three inboxes is stored once. Postgres holds metadata, extracted text and
  vectors.
- **Tenancy in depth.** Every row carries `workspace_id`; queries filter by
  it, and Postgres row-level security enforces it as a second line. Large
  tables are partitioned by workspace hash when volume calls for it.
- **Clients follow a change feed.** Each client holds a cache per workspace
  and subscribes to a cursor-based feed (server-sent events) of changes the
  member may see, catching up from its cursor after a disconnect. Lists are
  not re-fetched on every screen; offline editing can be added on the same
  feed later.

## Consequences
- Self-hosting stays one container plus Postgres (plus object storage only if
  the self-hoster wants it off local disk).
- Projections cost storage and some lag (sub-second to seconds); screens that
  need read-your-own-writes read the owning module directly.
- Row-level security needs the workspace set on every connection
  (`SET LOCAL`), which the kernel's database layer does for every request and
  job.

## Alternatives considered
- Separate services per role or per module — operational weight self-hosters
  shouldn't carry; the role split gives the scaling benefit without it.
- A dedicated broker (Kafka, NATS) — another system to run; Postgres handles
  the expected volume and can be swapped behind the queue interface later.
- Blobs in Postgres — simple, but bloats backups and the buffer cache.
