# 0002 — Rust modular monolith of vertical-slice modules

**Status:** Accepted · 2026-10-12

## Context
The product spans several domains (mail, calendar, tasks, files, accounting,
and possibly user-defined models and workflows). It must be easy to self-host
(one process, one database) yet easy to extend without every feature touching
shared layers.

## Decision
- **One Rust server** (axum + tokio) composed from **module crates** under
  `crates/modules/<domain>`. Each module exposes a single `module()` value
  (routes + OpenAPI + event handlers) that `campbooks-server` mounts.
- **Vertical slices inside modules:** each use case lives in
  `src/features/<feature>/` with its handler, types, queries and tests. Shared
  module-internal types go to `src/domain/`.
- **Hard boundaries:** modules depend only on `campbooks-kernel` and
  `campbooks-ai`. Cross-module communication is via domain events, or an
  allowlisted dependency in `architecture.toml`. Enforced by `cargo xtask arch`.
- **Data ownership:** PostgreSQL (with pgvector) and one schema per module. No
  cross-schema joins or foreign keys. sqlx with compile-time-checked queries;
  the offline query cache `.sqlx/` is committed.
- **Contract:** every endpoint is documented with utoipa; `openapi/openapi.json`
  is generated and committed, and clients generate their types from it.

## Consequences
- A module can later be extracted into a service with little change, but we
  pay for that only if we need it.
- Cross-domain screens (a "today" view, search) compose in the client or via
  event-fed projections, never via SQL joins across modules.
- Background jobs need a durable queue; the choice (Postgres-backed) is a
  follow-up decision.

## Alternatives considered
- Microservices — far too much operational weight for self-hosters.
- Layered monolith (controllers/services/repos) — what made the Rails app hard
  to change; every feature touches every layer.
