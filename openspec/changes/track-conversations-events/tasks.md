# Tasks

## 1. The plan

- [ ] 1.1 `tracking/patches/0002-conversations.yaml`: Changelog row (plan version 2), the 40 events, their properties, the records `conversations.draft`, `.need`, `.rule`, `.rule_run`, their enums, and a Coverage row for every operation, job, schedule and handler of the module
- [ ] 1.2 `apply`, `generate`, `check`; commit the workbook, the mirror and the generated code

## 2. Tracking

- [ ] 2.1 `domain::tracked`: the snapshots (conversation, draft, need, rule, rule run) read from the module's rows, and the enum mappings
- [ ] 2.2 Flags (`domain::flags::apply`): read, unread, star, unstar, trash, untrash, spam, not spam
- [ ] 2.3 `move_to_stream` (with `before`), `add_label`, `remove_label`
- [ ] 2.4 Needs: resolve, confirm, reopen
- [ ] 2.5 Drafts: create, update, delete, send, cancel; jobs `release_send`, `check_send`; handler `record_send`
- [ ] 2.6 Rules: create, confirm, decline, update, remove, restore, the replaced rules; `apply_rule`'s run summary
- [ ] 2.7 Rule notices, rule suggestions, person notes
- [ ] 2.8 `apply_changes` batch summary; the model pass; Scout's reads and their request
- [ ] 2.9 `create_inbox`, `forget_connection`

## 3. Tests

- [ ] 3.1 A test per live event reading it back from `kernel.tracked_events` (name, version, actor, workspace, operation, snapshot), and that a no-op tracks nothing
- [ ] 3.2 The server's coverage test lists nothing of `conversations`

## 4. Last

- [ ] 4.1 OpenAPI regenerated (unchanged), client types unchanged
- [ ] 4.2 Tracking plan regenerated
- [ ] 4.3 Changelog entry file (`changelog/unreleased/added/325-track-conversations.md`)
- [ ] 4.4 Gates green in CI
