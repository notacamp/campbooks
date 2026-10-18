# Tasks

## 1. Export

- [x] 1.1 `export_set` takes the member's access and an optional inbox; `visible_inboxes` bounds it; an unreadable inbox is 404
- [x] 1.2 `workspaces::operator::members_named`: a member by id or verified address
- [x] 1.3 CLI: `--member` required, `--inbox` optional, clear errors (nobody, several, an unreadable inbox)

## 2. Lifecycle

- [x] 2.1 The `.campbooks-labelling` marker; the absolute path and a purge reminder on finish
- [x] 2.2 `labelling purge --out`: marker required, no links, nothing else removed

## 3. Eval

- [x] 3.1 A committed set under `crates/ai/evals/` is never `private`

## 4. Tests

- [x] 4.1 A member never exports a teammate's private inbox (both sets); a member reading nothing exports nothing
- [x] 4.2 An inbox the member cannot read is refused; `--member` is required
- [x] 4.3 Purge refuses a folder without the marker, a foreign marker, a file and a link; removes an export
- [x] 4.4 Member lookup by id and verified address; the dataset guard
- [x] 4.5 CHANGELOG; gates green in CI
