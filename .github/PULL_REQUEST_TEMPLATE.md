<!--
  PR title must follow Conventional Commits — it becomes the squash-merge commit
  on main. e.g.  feat(skim): add bulk archive   |   fix: stop thread split
  See CONTRIBUTING.md → "PR titles follow Conventional Commits".
-->

## What & why

<!-- What does this change, and why? Link any issue: "Closes #123". -->

## Type of change

- [ ] `feat` — new feature
- [ ] `fix` — bug fix
- [ ] `docs` — documentation only
- [ ] `refactor` / `chore` — internal, no user-facing behavior change
- [ ] Breaking change (explain the migration path below)

## How was this tested?

<!-- Tests added/updated, and manual steps. For UI, attach before/after
     screenshots at BOTH mobile (375px) and desktop width. -->

## Checklist

- [ ] 🔒 **No secrets, keys, IPs, hostnames, account IDs, or real names/emails**
      in the diff, commits, or screenshots (this is a public repo — see
      [CONTRIBUTING.md](../CONTRIBUTING.md#-first-the-one-rule-that-matters-most-keep-this-repo-clean)).
- [ ] CI is green locally: `bin/rubocop`, `bin/brakeman --no-pager`,
      `bin/bundler-audit`, `bin/importmap audit`, `bin/rails db:test:prepare test`.
- [ ] Commits are **signed** — `main` requires verified signatures
      ([how to set it up](../CONTRIBUTING.md#signed-commits)).
- [ ] Tests added/updated for the change.
- [ ] [`CHANGELOG.md`](../CHANGELOG.md) updated under `[Unreleased]`
      (or N/A: internal-only change).
- [ ] UI changes use Phlex components, have a Lookbook preview, and were verified
      at **mobile (375px) and desktop** width.
- [ ] New user-facing strings are translated in **en/pt/es/fr**
      (`bundle exec i18n-tasks missing` is clean).

> ⚠️ Merging to `main` deploys this commit to **production with no rollback**.
> Make sure it's production-ready.
