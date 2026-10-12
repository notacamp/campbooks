# Contributing

The rules are in [`AGENTS.md`](AGENTS.md); this is the walkthrough.

## Your first PR, end to end

1. **Pick or open an issue.** Rewrite tasks are tracked by the maintainers;
   outside contributors open an issue here first so we can agree on scope.
2. **Branch off `next`:** `feat/<issue>-short-slug` (`fix/`, `docs/`,
   `refactor/`, `chore/` likewise).
3. **Write the spec first:** `/opsx:propose <idea>` (or create
   `openspec/changes/<name>/` by hand: proposal, design, tasks, spec deltas).
   Get it reviewed before writing code if the change is more than small.
4. **Implement** inside the owning module (`crates/modules/<domain>/src/features/<feature>/`,
   `clients/app/src/modules/<domain>/`). Adding a module or a cross-module
   dependency needs an ADR or an `architecture.toml` entry.
5. **Run the gates** (all must pass):
   ```sh
   cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace && cargo xtask arch && cargo xtask openapi --check
   cargo deny check && cargo machete
   pnpm --dir clients lint typecheck test deps:check deadcode format:check api:check i18n:check
   ```
6. **Changelog:** add a line under `[Unreleased]` in `CHANGELOG.md` for
   anything user-visible (`feat/` and `fix/` branches are checked).
7. **Open a PR into `next`.** Title = Conventional Commit (it becomes the
   squash commit). Commits must be **signed** (SSH or GPG).
8. **UI changes:** verify at 375px and desktop, and **attach a screen recording**
   (see below). CI fails a UI PR without one.
9. **Any text a user sees** ships in English, Portuguese, Spanish and French
   with the same keys (CI runs `pnpm i18n:check`): European Portuguese,
   Spanish from Spain, full diacritics. Machine drafts are fine.
   Full rule, including third-party screens: AGENTS.md → Internationalization.

## Screen recordings

Every PR that changes the client UI shows the change working, at 375px and on
desktop, in a `## Recording` section of the PR body.

1. Start the app (`pnpm --dir clients dev`) with **demo data only** — never a
   real inbox, real documents or anyone's face.
2. Write or reuse a scenario in `clients/app/recordings/scenarios/` and run
   `pnpm --dir clients record <scenario>`; it writes a 375px and a desktop video.
3. Attach them: drag the files into the PR description (GitHub hosts them).
   Maintainers use the `upload-media` helper, which prints ready-to-paste
   markdown with an inline preview.

A change under `clients/app/src` that is genuinely invisible (a refactor with
no visual effect) can be exempted by a maintainer with the `no-recording` label.

## Releases

Versions follow semver from the `VERSION` file. Until Next replaces the Rails
app, its releases live in their own namespace so they never collide with the
Rails `v0.x.y` tags and releases that `main` owns: tags `next-vX.Y.Z`, images
`ghcr.io/notacamp/campbooks:next-vX.Y.Z` and `:next-vX.Y`.

Every green push to `next` already publishes `:next` and `:next-<sha12>` and
deploys staging (`publish-next.yml`). A *release* is the same image under a
version, plus a tag and a GitHub **prerelease**.

**To cut a release**, open one PR into `next` (title `chore: release next-vX.Y.Z`)
that:

1. sets `VERSION` to `X.Y.Z` and `[workspace.package] version` in `Cargo.toml`
   to match (`cargo xtask arch` checks this; refresh `Cargo.lock`);
2. renames `## [Unreleased]` in `CHANGELOG.md` to `## [next-vX.Y.Z] - YYYY-MM-DD`
   and adds a fresh empty `## [Unreleased]` above it.

The Policy check fails the PR if the changelog section is missing or the tag
already exists. Merge it; when CI is green on the merge commit,
`release-next.yml` builds the image, tags it `next-vX.Y.Z` and `next-vX.Y`,
fails on fixable HIGH/CRITICAL Trivy findings, attests build provenance and an
SPDX SBOM, and only then creates the tag and the prerelease (the changelog
section is the release notes). Nothing is committed back to `next`: its branch
ruleset requires checks on every commit, so the changelog is rolled in the PR.
If the workflow fails after the merge, fix the cause and re-run it from the
Actions tab (`Release next`, branch `next`); it is idempotent until the tag exists.
Verify an image with `gh attestation verify oci://ghcr.io/notacamp/campbooks:next-vX.Y.Z --repo notacamp/campbooks`.

Releases do not deploy anything yet (staging follows `:next`); the workflow
documents where a production deploy dispatch will hook in.

## Working with AI agents

Agents follow exactly the same rules. `CLAUDE.md` imports `AGENTS.md`; the
OpenSpec commands are in `.claude/commands/opsx/`. One change = one worktree =
one session = one PR.
