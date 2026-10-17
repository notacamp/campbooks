# Tasks

## 1. Order by a counter

- [x] 1.1 Identity migration: `identity.accounts.suspension_seq` (0, or 1 for a suspended account)
- [x] 1.2 `suspend` / `unsuspend` bump it under the account's lock; the events carry `seq`
- [x] 1.3 Workspaces and conversations read `seq`; the higher wins, none is older, `at` orders the rest
- [x] 1.4 Conversations migration: `member_suspensions.seq`; the upsert and `forget_member` follow it

## 2. The fan-out races

- [x] 2.1 Workspaces migration: the newest change on `workspaces.users`
- [x] 2.2 The fan-out locks the user's row (`FOR NO KEY UPDATE`), records the change, lists the memberships, publishes, then commits
- [x] 2.3 Accepting, creating a workspace and departing lock the user's row first; accepting announces the recorded change, creating a recorded suspension

## 3. Dead deliveries

- [x] 3.1 Kernel `jobs::Watch` on jobs and handlers, carried and checked by the registry
- [x] 3.2 The worker logs the watched line at WARN when a job dies (failed for good, or its last lease expired)
- [x] 3.3 `GET /admin/v1/system/jobs` lists `watched`; the four suspension handlers are `suspension_fanout`
- [x] 3.4 Admin → Jobs: "Work to watch", in four languages

## 4. Keeping a paused author's proposal

- [x] 4.1 `RuleView.paused_once_kept`
- [x] 4.2 The rules list's note before Keep this rule and the paused toast, in four languages

## 5. Tests

- [x] 5.1 Identity: each change carries the next count; a repeat counts nothing
- [x] 5.2 Workspaces: ordering by the counter; joining while being suspended and leaving while a lift is announced, each with two connections; coming back after a missed lift
- [x] 5.3 Conversations: a later count wins over an earlier clock; replays; changes from before the counter; leaving keeps a counted suspension; `paused_once_kept` and confirming
- [x] 5.4 Kernel: watched kinds counted and logged, conflicting watches refused
- [x] 5.5 Clients: the note and toast; the Jobs panel's work to watch

## 6. Wrap-up

- [x] 6.1 Migrations, backfill and rollback checked on a throwaway database (rolled back: identical columns, constraints and `_sqlx_migrations` to the base)
- [x] 6.2 OpenAPI, admin API, events catalogue and client types regenerated
- [x] 6.3 CHANGELOG updated
- [x] 6.4 Gates green (CI)
- [x] 6.5 UI verified at 375px and desktop (recording)
