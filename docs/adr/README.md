# Architecture Decision Records

One file per decision, numbered, never edited after acceptance except to mark
it superseded. Disagree? Write a new ADR that supersedes the old one.

| # | Decision | Status |
| --- | --- | --- |
| [0001](0001-spec-driven-development.md) | Spec-driven development with OpenSpec | Accepted |
| [0002](0002-modular-monolith-vertical-slices.md) | Rust modular monolith of vertical-slice modules | Accepted |
| [0003](0003-one-ui-tauri-everywhere.md) | One React/TypeScript UI, shipped everywhere with Tauri 2 | Accepted |
| [0004](0004-oauth-dedicated-idp-bff.md) | OAuth 2.1 with a dedicated IdP; BFF for web, keychain for native | Accepted (IdP pick pending spike) |
| [0005](0005-open-weight-ai-by-role.md) | Open-weight AI only, addressed by role, fully user-configurable | Accepted |
| [0006](0006-one-image-docker-compose.md) | One image; docker compose for self-hosters | Accepted |
| [0007](0007-quality-gates-supply-chain.md) | Deterministic quality and supply-chain gates, no AI review in CI | Accepted |
| [0008](0008-platform-ready-typed-core.md) | Typed core now, platform later, seams from day one | Proposed |
| [0009](0009-decide-role-typed-decision-model.md) | The `decide` role is a typed decision model (Laya) | Accepted |
| [0010](0010-connectors-and-change-streams.md) | Every outside source is a connector producing a change stream | Proposed |
| [0011](0011-scaling-shape.md) | One binary in three roles; projections from an event log; blobs outside Postgres | Proposed |
| [0012](0012-postgres-job-queue.md) | Our own Postgres job queue; events fan out to jobs at publish time | Proposed |

Template: Context → Decision → Consequences → Alternatives considered.
