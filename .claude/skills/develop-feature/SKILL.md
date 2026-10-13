---
name: develop-feature
description: Take a Campbooks task from issue to pull request - size it, write or update the OpenSpec change, implement it as a vertical slice, run the gates, verify it in the running app, and open the PR (with a screen recording for UI changes). Use after bootstrap-task, or whenever asked to implement, build or ship a feature or fix in this repo.
---

The working loop for one change. The rules are in `AGENTS.md`; this skill is
the order to apply them in. Work inside the task's worktree
(`bootstrap-task`).

## 1. Size it

Read the issue, its OpenSpec change if one exists, and the code it touches.
Decide:

- **Small** (one slice, no new endpoint, scope, table or setting): a short
  change with one requirement is enough.
- **Normal**: a full change (proposal, design, tasks, spec deltas).
- **Too big** for one PR (more than a few slices, or more than one module's
  behaviour): split it. Propose the split and ask before going on.

A new module goes through the `add-module` skill first.

## 2. Spec first

AGENTS.md → Spec-driven workflow. Create or update
`openspec/changes/<name>/` (the `openspec-propose` skill or `/opsx:propose`)
with the tracking issue URL in `.openspec.yaml`, a Non-goals section in the
proposal, and in the design: data ownership, events, and the scope of every
endpoint. Validate:

```sh
npx -y @fission-ai/openspec@1.13.2 validate --all --strict --no-interactive
```

Put open questions for a human in `openspec/decisions-needed.md` with a
recommended answer instead of guessing.

## 3. Implement slice by slice

AGENTS.md → Architecture rules and Security rules.

- A new use case: `cargo xtask new-feature <module> <feature>` scaffolds the
  slice (handler, OpenAPI path with its security, `operationId` and
  `x-campbooks` metadata, tests) and mounts it. Replace its placeholder and
  every `TODO(new-feature)`; the metadata rules are ADR 0013.
- A new event: derive `utoipa::ToSchema`, declare it with
  `Background::publishes`, then `cargo xtask events`.
- Write the test first where you can; database tests use `#[sqlx::test]`.
- New `sqlx::query!` macros: refresh the offline cache with
  `cargo sqlx prepare --workspace -- --all-targets` (needs `DATABASE_URL`).
- Endpoints changed: `cargo xtask openapi`, then
  `pnpm --dir clients api:generate`.
- Any text a user sees: all four locales (AGENTS.md → Internationalization).
- Tick the change's `tasks.md` as you go.

## 4. Gates

All of AGENTS.md → Code quality must pass; the commands are in
`CONTRIBUTING.md` step 5. With `DATABASE_URL` set to your worktree's database:

```sh
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace && cargo xtask arch && cargo xtask openapi --check && cargo xtask events --check
cargo deny check && cargo machete
```

Plus each client gate (`pnpm --dir clients <gate>`) if `clients/` changed. Fix
the cause of a failure; never silence a lint or skip a test.

## 5. Verify it works

Run the app (`cargo run -p campbooks-server`, `pnpm --dir clients dev`) and
use the change the way a user would. UI: check it at 375px and on desktop,
then record it (`pnpm --dir clients record <scenario>`, demo data only;
CONTRIBUTING.md → Screen recordings).

## 6. Pull request

- `CHANGELOG.md` → `[Unreleased]` for anything users or self-hosters notice.
- Signed commits; if `next` moved, `git merge origin/next` (never rebase or
  force-push, `docs/parallel-work.md`).
- Push, then open the PR into `next` with a Conventional Commit title. Body:
  summary, design decisions, tests, follow-ups,
  `Tracking: notacamp/campbooks-cloud#<n>`, and `## Recording` with the video
  links (or "No UI change.").
- Comment the PR link on the issue and move the card to In Review
  (`docs/parallel-work.md` → Reporting on the board).
- Watch CI (`gh pr checks <pr> --watch`) and fix until green. Don't merge;
  maintainers do.
