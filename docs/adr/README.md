# Architecture Decision Records

One file per decision, numbered, never edited after acceptance except to mark
it superseded. Disagree? Write a new ADR that supersedes the old one.

| # | Decision | Status |
| --- | --- | --- |
| [0001](0001-spec-driven-development.md) | Spec-driven development with OpenSpec | Accepted |
| [0002](0002-modular-monolith-vertical-slices.md) | Rust modular monolith of vertical-slice modules | Accepted |
| [0003](0003-one-ui-tauri-everywhere.md) | One React/TypeScript UI, shipped everywhere with Tauri 2 | Accepted |
| [0004](0004-oauth-dedicated-idp-bff.md) | OAuth 2.1 with a dedicated IdP; BFF for web, keychain for native | Superseded by 0014 |
| [0005](0005-open-weight-ai-by-role.md) | Open-weight AI only, addressed by role, fully user-configurable | Accepted |
| [0006](0006-one-image-docker-compose.md) | One image; docker compose for self-hosters | Accepted |
| [0007](0007-quality-gates-supply-chain.md) | Deterministic quality and supply-chain gates, no AI review in CI | Accepted |
| [0008](0008-platform-ready-typed-core.md) | Typed core now, platform later, seams from day one | Accepted |
| [0009](0009-decide-role-typed-decision-model.md) | The `decide` role is a typed decision model (Laya) | Accepted |
| [0010](0010-connectors-and-change-streams.md) | Every outside source is a connector producing a change stream | Accepted |
| [0011](0011-scaling-shape.md) | One binary in three roles; projections from an event log; blobs outside Postgres | Accepted |
| [0012](0012-postgres-job-queue.md) | Our own Postgres job queue; events fan out to jobs at publish time | Accepted |
| [0013](0013-openapi-is-the-operation-catalogue.md) | The OpenAPI document is the operation catalogue | Accepted |
| [0014](0014-campbooks-owned-identity.md) | Campbooks owns identity: every owned mailbox is a way in, and Campbooks issues the tokens | Proposed |

Template: Context → Decision → Consequences → Alternatives considered.
