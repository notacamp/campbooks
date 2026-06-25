# Contributing to Campbooks

Thanks for your interest in Campbooks — the AI-native email client that declutters
your work. This guide is for **everyone who changes the code: human contributors
and AI agents alike** (Claude Code, Cursor, and friends). The same rules apply to
both. If you are an AI agent, also read the dedicated [section below](#working-as-an-ai-agent)
and the architecture conventions in [`CLAUDE.md`](CLAUDE.md).

Campbooks is **source-available** under the [Sustainable Use License](LICENSE).
By contributing you agree your contribution is licensed under the same terms.

---

## 🔒 First, the one rule that matters most: keep this repo clean

**This repository (`notacamp/campbooks`) is the PUBLIC core. Nothing private,
company-specific, or secret may ever land here — not in the code, the docs, OR
the git history.**

Before you open a PR, make sure your diff contains **none** of:

- Secrets, API keys, tokens, passwords, or private keys (real *or* realistic).
- Server IPs, hostnames, SSH details, account IDs, or deploy/infra specifics.
- A real person's name or email (use role addresses like `you@example.com`).
- Production runbooks, the deploy pipeline, or secrets-management details.

Instead:

- **Genericize.** Read configuration from `ENV` with neutral `example.com`
  defaults. Never hardcode infrastructure. The product name "Not A Camp" and the
  domain `not-a-camp.com` are fine (they're public). ⚠️ The GitHub org is
  **`notacamp`** (no hyphen) — distinct from the domain.
- Anything genuinely private belongs in the **private** ops repo, not here.

The public history is intentionally a single "initial public release" commit
because the original history held sensitive files. **Never force-push or publish
full personal history to `origin`.** When in doubt, leave it out and ask.

---

## Development setup

```bash
bin/setup        # install dependencies and prepare the database
bin/dev          # web + Tailwind + worker (Foreman), on http://localhost:3000
```

Seed login (from `db/seeds.rb`): `admin@example.com` / `changeme123`. The login
form is at `/session/new`. Full local/self-host details, including every
environment variable and integration, are in
[`docs/self-hosting.md`](docs/self-hosting.md).

---

## Branch & pull-request workflow

`main` is **protected and continuously deployed** — see
[Deployment](#deployment-merging-ships-to-production). All changes go through a
pull request. **Never commit directly to `main`.**

1. **Branch off `main`.** Use a short, prefixed name that matches the change:

   | Prefix     | For                                   |
   |------------|---------------------------------------|
   | `feat/`    | a new feature                         |
   | `fix/`     | a bug fix                             |
   | `docs/`    | documentation only                    |
   | `refactor/`| internal change, no behavior change   |
   | `chore/`   | tooling, deps, housekeeping           |

   e.g. `git checkout -b feat/bulk-archive-skim`.

2. **Keep the PR focused.** One logical change per PR. Smaller PRs are reviewed
   and shipped faster.

3. **Update the [`CHANGELOG.md`](CHANGELOG.md)** under `## [Unreleased]` in the
   same PR (skip for pure-internal `chore`/`refactor` with no user-visible
   effect).

4. **Make CI green.** Run the [quality gates](#quality-gates-run-before-you-push)
   locally before pushing.

5. **Open the PR** and fill in the template. CI runs automatically; a maintainer
   reviews; we **squash-merge** to `main`.

### PR titles follow Conventional Commits

We squash-merge, so **the PR title becomes the commit on `main`** — it must
follow [Conventional Commits](https://www.conventionalcommits.org/). Individual
commits on your branch can be free-form; only the **PR title** is strict.

```
<type>[optional scope]: <short, imperative description>
```

| Type       | Use for                          | Typical version bump |
|------------|----------------------------------|----------------------|
| `feat`     | a new user-facing feature        | minor                |
| `fix`      | a bug fix                        | patch                |
| `perf`     | a performance improvement        | patch                |
| `docs`     | documentation only               | none                 |
| `refactor` | internal change, no behavior     | none                 |
| `test`     | tests only                       | none                 |
| `chore`    | tooling, deps, build, CI         | none                 |

Mark a breaking change with `!` (e.g. `feat!: drop legacy webhook payload`) and
explain it in the PR body. See [Versioning](#versioning--releases) for what
"breaking" means here.

Examples:

```
feat(skim): add bulk archive action
fix(threading): stop multi-sender conversations splitting in the feed
docs(self-hosting): clarify FORCE_SSL behind a TLS proxy
```

---

## Quality gates (run before you push)

CI runs these on every PR; they **must pass before merge**. Run them locally to
get a green build the first time:

```bash
bin/rubocop                       # style / lint
bin/brakeman --no-pager           # Rails static security analysis
bin/bundler-audit                 # known CVEs in gems
bin/importmap audit               # known CVEs in JS deps
bin/rails db:test:prepare test    # the test suite
```

If you touched user-facing strings, also keep translations at parity:

```bash
bundle exec i18n-tasks missing    # every key present in en/pt/es/fr
bundle exec i18n-tasks health
```

---

## Coding conventions

The codebase has settled conventions — please match them rather than introduce a
parallel style. The sources of truth:

- **[`CLAUDE.md`](CLAUDE.md)** — architecture, subsystems, and the project-wide
  conventions. Read the relevant section before changing a subsystem.
- **UI must use Phlex components** from `app/components/campbooks/`. Don't write
  raw HTML+Tailwind in views when a component fits; if a reusable pattern has no
  component yet, extract one first. Every component needs a Lookbook preview.
  Full rules: [`docs/components.md`](docs/components.md).
- **Mobile-first & responsive** — no horizontal overflow down to 375px. Verify
  every UI change at mobile *and* desktop width before marking it done.
- **i18n** — the app ships in English, Portuguese, Spanish, and French at full
  key parity. New strings go through `t(".key")` and into all four locales.
- **Tests** — add or update tests for behavior changes. New features land with
  coverage; bug fixes land with a regression test.
- **Data safety** — never delete user data without explicit confirmation; never
  use `delete_all` on tables that may hold real records.
- **Voice** — user-facing copy stays consistent with
  [`docs/messaging.md`](docs/messaging.md).

---

## Versioning & releases

Campbooks follows [Semantic Versioning](https://semver.org). The single source of
truth for the current version is the [`VERSION`](VERSION) file at the repo root
(also reported at `/up` and in the Settings sidebar). Releases are git tags
`vX.Y.Z` with a matching [`CHANGELOG.md`](CHANGELOG.md) section and GitHub
Release.

Because Campbooks is an application (not a library), here's what each bump means:

- **MAJOR** (`X`) — a change that breaks **self-hosters or API consumers**:
  a removed/renamed environment variable or config default, a migration that
  needs manual steps, a breaking change to the public REST API (`/api/v1`) or the
  webhook contract, or dropping a provider.
- **MINOR** (`Y`) — backward-compatible additions: a new feature, integration,
  API endpoint/scope, or a new *optional* environment variable.
- **PATCH** (`Z`) — backward-compatible bug fixes, performance, docs, and
  internal refactors with no behavior change.

> **Pre-1.0:** while the version is `0.y.z`, minor releases may still contain
> breaking changes. We always call those out in the `CHANGELOG.md` (group them
> under **Changed** with a ⚠️) and in the PR.

### Cutting a release (maintainers)

1. Make sure `## [Unreleased]` in `CHANGELOG.md` is complete.
2. Bump [`VERSION`](VERSION) to the new `X.Y.Z`.
3. In `CHANGELOG.md`, move the `[Unreleased]` entries under a new
   `## [X.Y.Z] - YYYY-MM-DD` heading and update the compare links at the bottom.
4. Open a `chore(release): vX.Y.Z` PR and merge it (this deploys to prod).
5. Tag and push: `git tag -a vX.Y.Z -m "vX.Y.Z" && git push origin vX.Y.Z`.
6. Publish a **GitHub Release** from the tag, pasting that CHANGELOG section.

---

## Deployment (merging ships to production)

A merge to `main` triggers `.github/workflows/notify-deploy.yml`, which signals
the private cloud repo to build and deploy **that exact commit to production —
with no automatic rollback.** Forks never deploy (they lack the dispatch token).

Practically, this means **the bar for merging is "production-ready"**: CI green,
tested (including the UI at mobile and desktop), migrations safe to run on boot,
and no secrets in the diff. If you're not a maintainer, you don't deploy —
your job is a clean, green, well-described PR; a maintainer merges.

---

## Security

**Never report a vulnerability in a public issue or PR.** Follow the private
disclosure process in [`SECURITY.md`](SECURITY.md).

---

## Working as an AI agent

AI agents (Claude Code and others) are welcome here and follow **exactly the same
rules as humans** — plus a few that matter more when work is automated:

- **The public-repo rule is paramount.** Re-read the
  [first section](#-first-the-one-rule-that-matters-most-keep-this-repo-clean).
  Never write a secret, IP, hostname, account ID, or real person's name/email
  into code, docs, or commits. If a task seems to require infra/secrets, stop and
  flag it — that work belongs in the private ops repo, not here.
- **Always branch; never commit to `main`.** Open a PR like everyone else.
- **Use a Conventional Commits PR title** (see [above](#pr-titles-follow-conventional-commits)).
- **Update `CHANGELOG.md`** under `[Unreleased]` for any user-visible change.
- **Run the [quality gates](#quality-gates-run-before-you-push) locally** and make
  them pass *before* opening the PR — don't outsource a red build to CI.
- **Verify UI changes in a real browser** (e.g. Playwright) at **375px and
  desktop** width before reporting done. Screenshot both in the PR.
- **`CLAUDE.md` is your map** of the architecture and subsystem conventions —
  read the relevant section before editing, and follow it.
- **Never run destructive git or database operations** (force-push, history
  rewrite, `delete_all`, dropping data) without explicit human confirmation.
- **Explain what you changed and why** in the PR body — reviewers (and the next
  agent) rely on it.

---

## Code of Conduct

Participation is governed by our [Code of Conduct](CODE_OF_CONDUCT.md). Be kind.

Thanks for helping make Campbooks better. 💛
