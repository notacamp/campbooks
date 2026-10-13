---
name: bootstrap-task
description: Start work on a Campbooks tracking issue - read the issue, create a worktree and a `feat/<issue>-<slug>` branch off `next`, give it its own test database, and mark the issue In Progress. Use when asked to pick up, start or bootstrap an issue or task by number or URL.
---

Set up an isolated place to work on one tracking issue. Background and the
reasons behind each step: `docs/parallel-work.md`.

**Input:** an issue number or URL from `notacamp/campbooks-cloud`. Ask for it
if you don't have one; never invent one.

## Steps

1. **Read the issue.**
   ```sh
   gh issue view <n> -R notacamp/campbooks-cloud --comments
   ```
   Note its checklist, its parent and anything it is blocked by. If it is
   blocked, or assigned to someone else, stop and say so.

2. **Pick the branch name**: `feat/<n>-<slug>` (`fix/`, `docs/`, `refactor/`,
   `chore/` when that fits better, AGENTS.md → Workflow). `<slug>` is two to
   four kebab-case words from the title.

3. **Create the worktree** next to the current checkout, from the latest `next`:
   ```sh
   git fetch origin
   git worktree add -b <branch> ../campbooks-<n>-<slug> origin/next
   ```
   From here on, work only inside that directory. If you are already in a
   worktree made for this issue, reuse it.

4. **Give it a database** on the dev Postgres (port 55432, see
   `compose.dev.yaml`); never use the shared `campbooks` database:
   ```sh
   db="campbooks_$(printf %s "<branch>" | tr -c 'a-zA-Z0-9' '_' | tr 'A-Z' 'a-z')"
   PGPASSWORD=postgres createdb -h localhost -p 55432 -U postgres "$db"
   url="postgres://postgres:postgres@localhost:55432/$db"
   printf 'DATABASE_URL=%s\nCAMPBOOKS_DATABASE_URL=%s\n' "$url" "$url" > .env
   DATABASE_URL="$url" cargo sqlx migrate run --source migrations
   ```
   Export both variables in every shell you run tests or the app from, and
   migrate again after adding or merging a migration (`sqlx::query!` checks
   queries against this database at compile time).

5. **Install the clients** if the task touches `clients/`:
   `pnpm --dir clients install --frozen-lockfile`.

6. **Move the issue to In Progress.** If `CAMPBOOKS_BOARD_HELPER` points at an
   executable, run `"$CAMPBOOKS_BOARD_HELPER" <n> "In Progress"`. Otherwise use
   `gh project item-edit` if you can, or mention it in your report.

7. **Find the change.** If `openspec/changes/` already holds a change whose
   `.openspec.yaml` names this issue, read it: it is the plan. If not, the
   next step is writing one.

## Report

The worktree path, branch, database name, the change (existing or "none
yet"), and anything that blocks the issue. Continue with the
`develop-feature` skill, or `add-module` if the issue adds a module.
