# Parallel work: one worktree per task

Several people and agent sessions work on Campbooks at the same time. They
stay out of each other's way with a few habits: every task gets its own git
worktree, branch, database and PR. The rules themselves are in
[`AGENTS.md`](../AGENTS.md); this page is the how-to.

## One task, one worktree, one branch

```sh
git fetch origin
git worktree add -b feat/123-thread-view ../campbooks-123-thread-view origin/next
cd ../campbooks-123-thread-view
```

- **Branch names** follow AGENTS.md → Workflow: `feat/<issue>-<slug>`,
  `fix/…`, `docs/…`, `refactor/…`, `chore/…`, where `<issue>` is the tracking
  issue's number and `<slug>` a few kebab-case words.
- **One session per worktree.** Two sessions in one checkout overwrite each
  other's files, builds and branch switches. Starting something else means
  starting another worktree.
- Each worktree builds into its own `target/` and installs its own
  `clients/node_modules` (`pnpm --dir clients install` is quick: pnpm
  hard-links from its shared store).
- Clean up after the PR merges: `git worktree remove ../campbooks-123-thread-view`
  and `git branch -d feat/123-thread-view`.

The `bootstrap-task` skill in `.claude/skills/` does all of this for an agent
session.

## One database per worktree

The development Postgres runs once, from `compose.dev.yaml`, on port 55432.
Its `campbooks` database belongs to the main checkout and tracks `next`.
Every other worktree gets a database of its own:

```sh
branch=$(git branch --show-current)
db="campbooks_$(printf %s "$branch" | tr -c 'a-zA-Z0-9' '_' | tr 'A-Z' 'a-z')"
PGPASSWORD=postgres createdb -h localhost -p 55432 -U postgres "$db"
export DATABASE_URL="postgres://postgres:postgres@localhost:55432/$db"
export CAMPBOOKS_DATABASE_URL="$DATABASE_URL"
cargo sqlx migrate run --source migrations    # sqlx-cli
```

`feat/123-thread-view` gets `campbooks_feat_123_thread_view`. Put the
exports in a `.env` at the worktree root (git ignores it) or in your shell.

- **Compiling:** with `DATABASE_URL` set, `sqlx::query!` checks each query
  against that database, so keep it migrated (rerun `cargo sqlx migrate run`
  after adding or merging a migration), or compile like CI with
  `SQLX_OFFLINE=true`. `cargo sqlx prepare --workspace -- --all-targets`
  needs the migrated database and `SQLX_OFFLINE` unset.

- **Tests:** `#[sqlx::test]` creates and drops a throwaway database per test
  on the server `DATABASE_URL` points at, so tests never touch your
  worktree's data.
- **Running the app** migrates `CAMPBOOKS_DATABASE_URL`, which is your
  worktree's database.
- **Never migrate the shared `campbooks` database from a worktree.** A
  branch's migration that changes before it merges, or never merges, leaves
  that database in a state `next` cannot reproduce, and every other checkout
  then fails at startup ("migration … was previously applied but is
  missing"). If it happened, drop your worktree's database and recreate it.
- Drop your database when the worktree goes: `dropdb -h localhost -p 55432 -U postgres "$db"`.

## Running two apps at once

The API listens on 8080 and the web dev server on 5173. In a second worktree,
move both:

```sh
CAMPBOOKS_BIND_ADDR=127.0.0.1:8180 cargo run -p campbooks-server
CAMPBOOKS_API_URL=http://localhost:8180 pnpm --dir clients dev --port 5273
```

## Keeping up with `next`

Merge, don't rebase:

```sh
git fetch origin
git merge origin/next
```

PRs are squash-merged, so a merge commit on your branch costs nothing.
Rebasing a pushed branch needs a force-push, which rewrites history other
sessions or reviewers may have fetched, so we don't force-push. Resolve
conflicts in the merge, rerun the gates, push.

Merge early when `next` touches the files you share: the module list in
`crates/server/src/lib.rs`, `Cargo.toml`, `Cargo.lock`, `openapi/openapi.json`
and `clients/app/src/app/`. Regenerate the generated files after the merge
(`cargo xtask openapi`, `pnpm --dir clients api:generate`) instead of
hand-merging them.

## Reporting on the board

Tasks are GitHub issues in the maintainers' tracker, grouped on a project
board. Whoever works on an issue, human or agent, keeps its card honest:

| When | Board status | Also |
| --- | --- | --- |
| You start | In Progress | |
| The PR is open | In Review | Comment on the issue with the PR link. |
| The PR merges | Done | Maintainers move it; don't do it yourself. |

`gh project item-edit` moves a card. If your setup provides a helper script,
point `CAMPBOOKS_BOARD_HELPER` at it (it takes `<issue> "<status>"`) and the
skills in `.claude/skills/` call it for you. Without either, say in the PR
which status the card should have.
