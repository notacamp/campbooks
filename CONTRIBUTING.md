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
   dependency needs an ADR or an `architecture.toml` entry. Scaffold instead
   of copying by hand (see [Generators](#generators)).
5. **Run the gates** (all must pass):
   ```sh
   cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace && cargo xtask arch && cargo xtask openapi --check && cargo xtask events --check
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

## Generators

```sh
cargo xtask new-module <name> [--issue <tracking issue URL>]
cargo xtask new-feature <module> <feature>
```

`new-module` creates a module in the shape of the reference module
(`system`): the crate with its scopes (`<name>:read`, `<name>:write`), an
events file and an `overview` read slice with tests; the schema migration;
the OpenSpec change `add-<name>-module`; and the client module with a
placeholder page and its four locale files. It registers the module in
`Cargo.toml`, the server and the client shell, and refuses to overwrite
anything. `new-feature` adds a slice (a scoped `GET` handler, its OpenAPI path
and tests) and mounts it on the module's router. Both print what to do next;
the `TODO(new-module)` / `TODO(new-feature)` markers show what to fill in, and
a generated OpenSpec change fails `openspec validate --strict` until its spec
is written.

Templates are plain files in `xtask/templates/`. After changing one, review
and refresh the snapshot (`UPDATE_SNAPSHOTS=1 cargo test -p xtask`) and run
`cargo xtask scaffold-check --full`, which generates a module and a feature
in a temporary copy of the repo and runs the Rust and client gates on them
(CI runs the Rust part).

## Local hooks (optional)

[lefthook](https://lefthook.dev) runs the fast gates before CI does:
`cargo fmt --check`, prettier on staged client files and `gitleaks` (when
installed) on commit; clippy, `cargo xtask arch` and the client lint on push.
They are opt-in and never required: CI stays the source of truth.

```sh
brew install lefthook        # or: go install github.com/evilmartians/lefthook/v2@latest
lefthook install             # once per clone; all worktrees share the hooks
```

Skip them once with `LEFTHOOK=0 git push`, or one job with
`LEFTHOOK_EXCLUDE=clippy git push`. Branches without a `lefthook.yml` are
unaffected. Remove them with `lefthook uninstall`.

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

Every push to `next` builds, scans and attests the image in CI as
`:ci-<sha>`; once CI is green, `publish-next.yml` tags that same image
`:next` and `:next-<sha12>` and deploys staging and production. A *release*
is the same recipe under a version, plus a tag and a GitHub **prerelease**.

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
OpenSpec commands are in `.claude/commands/opsx/`, and the task skills in
`.claude/skills/`: `bootstrap-task` (issue → worktree, branch, database),
`develop-feature` (size → spec → slice → gates → verify → PR) and
`add-module`. One change = one worktree = one session = one PR; how to run
several side by side is in [`docs/parallel-work.md`](docs/parallel-work.md).
