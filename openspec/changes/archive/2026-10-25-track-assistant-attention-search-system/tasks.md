# Tasks

## 1. The plan

- [ ] 1.1 `tracking/patches/0007-assistant-attention-search-system.yaml`: Changelog row (plan version 7), the 15 events, their properties, the records `assistant.chat`, `.answer`, `.proposal`, `.profile`, `attention.item`, `kernel.job`, their enums, and a Coverage row for every mutating operation, job, schedule and handler of the four modules
- [ ] 1.2 `apply`, `generate`, `check`; commit the workbook, the mirror and the generated code; include `tracking.gen.rs` from the `assistant`, `attention` and `system` crates' `lib.rs`

## 2. Tracking

- [ ] 2.1 assistant: the snapshots (chat, answer, proposal, profile) and enum mappings; chat started, continued, answered, deleted; proposals confirmed, declined, undone; profile updated (the same values track nothing); a streamed answer runs in the request's tracking context
- [ ] 2.2 attention: item snoozed (the same Later again tracks nothing) and unsnoozed (nothing to end tracks nothing), with `before`
- [ ] 2.3 kernel: `jobs::admin` takes the tracker and tracks a job retried, jobs retried (one summary), a job discarded, a schedule's run discarded; nothing that changed nothing
- [ ] 2.4 system: the admin job actions pass the tracker; the AI meter, composed with the server's tracker, tracks the day's first refusal

## 3. Tests

- [ ] 3.1 The assistant and attention test kits write the outbox on demand
- [ ] 3.2 A test per live event reading it back from `kernel.tracked_events` (name, version, actor, workspace, operation, snapshot), that a no-op tracks nothing, and that no body holds a question, an answer or an address
- [ ] 3.3 The server's coverage test lists nothing of `assistant`, `attention`, `search` or `system`

## 4. Last

- [ ] 4.1 OpenAPI regenerated (unchanged), client types unchanged
- [ ] 4.2 Tracking plan regenerated
- [ ] 4.3 Changelog entry file (`changelog/unreleased/added/330-track-assistant-attention-search-system.md`)
- [ ] 4.4 Gates green in CI
