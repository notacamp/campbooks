# 0007 — Deterministic quality and supply-chain gates, no AI review in CI

**Status:** Accepted · 2026-10-12

## Context
Much code will be written by agents. Patterns must be enforced mechanically,
and the public repo must stay free of secrets and vulnerable dependencies.

## Decision
CI runs only deterministic, binary pass/fail gates:
- **Rust:** rustfmt, clippy (pedantic, `-D warnings`), tests, `cargo deny`
  (advisories, licenses, bans incl. no OpenSSL, crates.io only),
  `cargo machete`, `cargo xtask arch` (module boundaries + migration naming),
  `cargo xtask openapi --check`, `cargo sqlx prepare --check`.
- **TypeScript:** ESLint (no `eslint-disable`), strict typecheck, Vitest with
  coverage thresholds, dependency-cruiser boundaries, knip dead code,
  Prettier, `pnpm audit --prod`, generated-API drift check.
- **Repo:** gitleaks secret scan, Trivy image scan, OpenSpec validation,
  Conventional Commit PR title, changelog entry on `feat/` and `fix/`.
- **Dependencies:** Dependabot (cargo, npm, actions, docker), grouped, monthly.
- **No AI code review in CI.**

## Consequences
- A pattern we care about gets a lint rule, an xtask check or a test; prose
  rules without enforcement are flagged as such in AGENTS.md.
- Gates must stay fast; slow suites move to scheduled workflows.
