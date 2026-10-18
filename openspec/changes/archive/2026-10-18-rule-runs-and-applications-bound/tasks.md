# Tasks

## 1. Runs

- [x] 1.1 `rule_runs::start` takes the member's `Access` and signs the run (`campbooks/conversations.rule-run/v1`: workspace, run, rule, generation, kind, inbox, requesting member); an apply run only while its rule is bound
- [x] 1.2 `rule_runs::bound`; `conversations.apply_rule` verifies the run at the rule's generation now before anything else acts (WARN with ids, cancelled, no retry)

## 2. Applications

- [x] 2.1 `rules::applications`: the MAC (`campbooks/conversations.rule-application/v1`: workspace, rule, inbox, conversation, run, `before`), `drop_unbound` before the merge and `seal` after it, both inside `effects::record`
- [x] 2.2 `rules::revert` verifies the row under its lock and leaves one that does not verify (WARN with ids); the run goes on

## 3. The deploy

- [x] 3.1 Migration `20261022010000`: `rule_runs.requested_by`, `rule_runs.mac`, `rule_applications.mac`; rollback SQL in `design.md`
- [x] 3.2 `conversations.remove_rule` with `restore=true` answers 409 `undo_unavailable` for a rule holding an unsigned application; OpenAPI and the client's generated types; the four catalogues

## 4. Tests

- [x] 4.1 Unit: every bound field of a run and of an application; a MAC copied to another row, run, rule, inbox or workspace; another key
- [x] 4.2 A planted revert run and forged application take nothing out of spam and request nothing; the member's own undo still skips the forged row
- [x] 4.3 Planted apply runs of a signed rule (unsigned, made up, copied from another run) are cancelled; a run of a rule signed again since stops
- [x] 4.4 An application changed with SQL, or carrying another row's MAC, is left while the rest go back, and the job does not fail
- [x] 4.5 An unsigned or undecodable application never fails ingest; it is dropped when the rule changes the conversation again, and undo answers `undo_unavailable` while any is held
