# 0006 — One image; docker compose for self-hosters

**Status:** Accepted · 2026-10-12

## Context
Self-hosting must stay simple, and the hosted cloud should run the same
artifact as everyone else.

## Decision
- One multi-arch (amd64/arm64) container image per release, built from the root
  `Dockerfile` (distroless runtime, non-root, built-in `healthcheck`
  subcommand, migrations embedded and run on boot).
- The web UI is served by the same server, so one image covers API and web.
- `deploy/compose.yaml` is the supported self-hosting path: server, Postgres
  with pgvector, the IdP, and an optional `ai` profile with local model servers.
- Upgrades are `docker compose pull && docker compose up -d`; migrations must
  be idempotent and forward-only. No manual upgrade steps, ever.
- The hosted cloud's deployment pipeline, infrastructure and cloud-only
  extensions live in a private repository and consume the published image.

## Consequences
- Desktop and mobile apps ship separately (store/notarised builds) and point
  at any server URL.
- Cloud-only behaviour (analytics, support widgets, extra telemetry) must be
  an extension point the private build fills, never code in this repo.

## Alternatives considered
- Separate web and API images — more moving parts for self-hosters.
- Kubernetes/Helm as the primary path — too heavy for the target audience.
