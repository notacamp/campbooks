# Require full tracking coverage

Modules: the tracking plan's tooling (`crates/tracking-plan/`), the server's
coverage test (`crates/server/src/tracking_tests.rs`), `connectors` (its
address checks) and the kernel's own maintenance (Coverage rows only), with
the tracking plan (`tracking/`) and CI (`.github/workflows/policy.yml`).
Tracking: https://github.com/notacamp/campbooks-cloud/issues/332, part of
epic https://github.com/notacamp/campbooks-cloud/issues/319.

## Why

ADR 0020 asks for a record of every action. The Coverage sheet is how the
plan proves it: every mutating operation, job, schedule and event handler
has a row naming the events it tracks, or why it tracks none. Until now a
missing row was only a warning, so that the modules could be instrumented
one at a time (#325 to #331, #349, #340). They all are. From here on a
missing row has to fail CI, or the record quietly stops covering
everything as features land.

The owner decided: no baseline and no ratchet. Every operation (in
`openapi.json` and `admin.json`), job, schedule and handler the server
composes has a row, or CI fails.

Three gaps are left on `next` and are closed here:

- `connectors.check_address` and its job `connectors.send_address_check`
  (campbooks-cloud#314) landed after connectors was instrumented;
- the kernel's own maintenance (its prune and sweep schedules, the change
  feed's sequencer and the re-key dispatcher), which no module PR owned.

## What Changes

- The plan tool's `check` fails on a mutating operation with no Coverage
  row (it was a warning). The report has no warnings left; the
  `warnings` field goes.
- The server's coverage test always fails on an operation, job, schedule
  or handler with no row (`REQUIRE_FULL_COVERAGE` goes), and says how to
  fix it.
- Tracking plan version 11 (`tracking/patches/0011-coverage-gaps.yaml`):
  - `connectors.address_check_requested`: a member asked for a mailbox's
    address to be checked again (`connectors.check_address`);
  - `connectors.address_check_send_completed`: the check's message went
    out, or could not be sent, with why (`connectors.send_address_check`);
  - Coverage rows for both, and exemptions for the kernel's maintenance.
- A Policy step, "Tracking plan changes come through a patch": a PR that
  changes `tracking/tracking-plan.xlsx` or `tracking/tracking-plan.json`
  also adds or changes a file under `tracking/patches/`.
- `AGENTS.md`, `docs/tracking.md`, ADR 0020's status line and the PR
  template say coverage is required, and how to read the failure.

No new endpoint, scope, migration, dependency or domain event. No change to
what any endpoint returns.

## Non-goals

- A baseline, an allow-list or a ratchet: the owner ruled them out.
- Tracking the address check's end in the stream's runs (the code found
  and the address verified, or not found in time): `connectors.sync_stream`
  is covered already, and those outcomes are a follow-up.
- Reads: they still need no row.
- Any personal data or content: no address, code, host or username.
