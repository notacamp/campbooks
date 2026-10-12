# AGENTS.md — rules for humans and agents

This file is the single source of truth for how work is done in this repo.
`CLAUDE.md` only imports it. Every rule names how it is enforced; a rule with
no enforcement is a convention you are still expected to follow.

## Read first

1. `docs/architecture.md` — the shape of the system (modules, crates, clients).
2. `docs/adr/` — decisions already taken. Don't re-litigate them in code; write
   a new ADR that supersedes one if you disagree.
3. `openspec/config.yaml` and `openspec/specs/` — the behaviour baseline.
4. `CONTRIBUTING.md` — branch, PR, changelog and release flow.

## 🔒 Public repository

This repo is public and source-available. **Nothing private lives here** — not
in code, docs, fixtures, or history: no secrets, IPs, hostnames of private
infrastructure, account IDs, or real people's names/emails. Hosted-cloud-only
behaviour, deploy pipelines and ops runbooks live in the private cloud repo.
Use `ENV` with neutral `example.com` defaults. *(gitleaks in CI.)*

## Spec-driven workflow

- **Specs before code.** Every behaviour change starts as an OpenSpec change
  (`openspec/changes/<name>/`: proposal, design, tasks, spec deltas). Use
  `/opsx:propose`, `/opsx:apply`, `/opsx:archive`. *(CI: `openspec validate --all --strict`.)*
- **One change = one branch = one PR.** The change records its tracking issue in
  `.openspec.yaml` (`issue: <url>`); never invent one.
- Unspecced behaviour and specs with no code behind them both block merge.
- Open questions for a human go in `openspec/decisions-needed.md`, one per
  heading, each with a recommended answer.

## Architecture rules (enforced)

- **Modular monolith, vertical slices.** A business domain is one crate under
  `crates/modules/<domain>`; inside it, each use case is a slice under
  `src/features/<feature>/` owning its handler, request/response types, queries
  and tests. No horizontal `controllers/`/`services/`/`repositories/` layers.
- **Modules don't import modules.** A module crate may depend on
  `campbooks-kernel` and `campbooks-ai` only. Cross-module needs go through
  domain events on the kernel event bus, or an explicit allowlist entry in
  `architecture.toml` (reviewed like an ADR). *(CI: `cargo xtask arch`.)*
- **Kernel knows no domain.** `campbooks-kernel` depends on no internal crate;
  `campbooks-ai` depends only on the kernel; only `campbooks-server` composes
  modules. *(CI: `cargo xtask arch`.)*
- **Each module owns a Postgres schema** named after it. Migrations live in
  `migrations/` named `<timestamp>_<module>__<description>.sql` and touch only
  that module's schema. No cross-schema joins or foreign keys — reference other
  modules by ID. *(CI: `cargo xtask arch` checks migration names.)*
- **Every HTTP endpoint is in the OpenAPI document** (`utoipa`), which is
  generated to `openapi/openapi.json` and committed. *(CI: `cargo xtask openapi --check`.)*
- **Errors are RFC 9457 problem+json** via `kernel::Error`. Permission failures
  on reads return 404, not 403.
- **Clients mirror the server.** `clients/app/src/modules/<domain>` maps 1:1 to
  `crates/modules/<domain>`. Modules import each other only through `index.ts`;
  `lib/` never imports `modules/`. *(CI: dependency-cruiser.)*
- **Clients never hand-write API types**; they come from `openapi/openapi.json`.
  *(CI: `pnpm api:check`.)*

## Security rules

- **Tokens never reach browser JavaScript.** The web client talks to the API
  through the server's BFF with an httpOnly, `SameSite=Strict` session cookie.
  Native (Tauri) clients keep tokens in the OS keychain on the Rust side; the
  webview calls the API via Tauri commands. See ADR 0004.
- The API is an OAuth 2.1 resource server: validate the JWT (issuer, audience,
  expiry, JWKS) and check a scope on every handler. *(Handler extractor
  `kernel::auth::Scoped<S>`; an endpoint without one fails `cargo xtask arch`
  once auth lands.)*
- No `unsafe` (`#![forbid(unsafe_code)]` in every crate, workspace lint).
- Untrusted HTML (email bodies) is rendered only in a sandboxed iframe with a
  strict CSP; never `dangerouslySetInnerHTML` outside `lib/ui/safe-html`.

## AI rules

- **Open-weight models only** in the shipped catalog. Every AI call goes through
  `campbooks-ai` by **role** (`decide`, `embed`, `complete`, optional
  `vision`), never by vendor. Providers speak the OpenAI-compatible HTTP API, so
  a self-hosted vLLM / llama.cpp / Ollama / TEI and a hosted provider serving
  open weights are interchangeable. Users and instance admins configure every
  role. See ADR 0005.
- AI output that changes user data is a **proposal** the user can undo, never a
  silent mutation.

## Code quality (all binary pass/fail in CI)

- Rust: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`
  (pedantic set in `Cargo.toml` `[workspace.lints]`), `cargo nextest run`,
  `cargo deny check` (advisories, licenses, bans, sources), `cargo machete`.
- TypeScript: `pnpm lint` (ESLint flat config, no `eslint-disable`),
  `pnpm typecheck` (strict + `noUncheckedIndexedAccess`), `pnpm test`,
  `pnpm deps:check`, `pnpm deadcode` (knip), `pnpm audit --prod`.
- Repo: gitleaks, Trivy on the built image, Conventional Commit PR title,
  changelog entry on `feat/` and `fix/` branches.

## Workflow

- Branch off `next` (the rewrite's trunk until cutover): `feat/<issue>-slug`,
  `fix/…`, `docs/…`, `refactor/…`, `chore/…`. PR title is a Conventional
  Commit; it becomes the squash commit. Commits must be signed.
- Tasks are tracked as GitHub issues in the private cloud repo; reference the
  issue URL in the PR body and in the change's `.openspec.yaml`.
- Update `CHANGELOG.md` `[Unreleased]` for user-visible changes.
- UI work: get a visual prototype approved before implementing it, then verify
  at **375px and desktop** before calling it done.
- Never delete user data without explicit confirmation; never send email to a
  real person from a dev or test environment.
