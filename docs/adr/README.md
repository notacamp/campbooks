# Architecture Decision Records

One file per decision, numbered, never edited after acceptance except to mark
it superseded. Disagree? Write a new ADR that supersedes the old one.

| # | Decision | Status |
| --- | --- | --- |
| [0001](0001-spec-driven-development.md) | Spec-driven development with OpenSpec | Accepted |
| [0002](0002-modular-monolith-vertical-slices.md) | Rust modular monolith of vertical-slice modules | Accepted |
| [0003](0003-one-ui-tauri-everywhere.md) | One React/TypeScript UI, shipped everywhere with Tauri 2 | Accepted |
| [0004](0004-oauth-dedicated-idp-bff.md) | OAuth 2.1 with a dedicated IdP; BFF for web, keychain for native | Accepted (IdP: Rauthy) |
| [0005](0005-open-weight-ai-by-role.md) | Open-weight AI only, addressed by role, fully user-configurable | Accepted |
| [0006](0006-one-image-docker-compose.md) | One image; docker compose for self-hosters | Accepted |
| [0007](0007-quality-gates-supply-chain.md) | Deterministic quality and supply-chain gates, no AI review in CI | Accepted |

Template: Context → Decision → Consequences → Alternatives considered.
