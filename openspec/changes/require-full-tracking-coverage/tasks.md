# Tasks

## 1. The gate

- [ ] 1.1 `crates/tracking-plan`: a mutating operation with no Coverage row is a problem of `check` (no more warnings); the tool's tests
- [ ] 1.2 `crates/server/src/tracking_tests.rs`: the coverage test always fails on an operation, job, schedule or handler with no row; no `REQUIRE_FULL_COVERAGE`
- [ ] 1.3 No "until #332" wording left in code or docs

## 2. The gaps

- [ ] 2.1 `tracking/patches/0012-coverage-gaps.yaml` (plan version 12): `connectors.address_check_requested`, `connectors.address_check_send_completed`, their enums, the Coverage rows of `connectors.check_address`, `connectors.send_address_check` and the kernel's maintenance
- [ ] 2.2 `apply`, `generate`, `check`; commit the workbook, the mirror and the generated code
- [ ] 2.3 Track both events in the transaction of the change
- [ ] 2.4 Tests that read both back from `kernel.tracked_events`, and that a no-op tracks nothing
- [ ] 2.5 Every other row the server's coverage test lists as missing in CI

## 3. CI and docs

- [ ] 3.1 Policy step "Tracking plan changes come through a patch"
- [ ] 3.2 `AGENTS.md`, `docs/tracking.md`, ADR 0020, the PR template

## 4. Last

- [ ] 4.1 OpenAPI unchanged, client types unchanged
- [ ] 4.2 Tracking plan regenerated
- [ ] 4.3 Changelog entry file (`changelog/unreleased/changed/332-tracking-gate.md`)
- [ ] 4.4 Gates green in CI
