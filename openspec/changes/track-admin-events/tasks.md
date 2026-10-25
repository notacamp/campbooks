# Tasks

## 1. The plan

- [ ] 1.1 `tracking/patches/0008-admin.yaml`: Changelog row (plan version 8), the events `admin.profile_granted` and `admin.profile_revoked`, their properties, the record `admin.profile` and the enum `admin.permission`
- [ ] 1.2 `apply`, `generate`, `check`; commit the workbook, the mirror and the generated code; include `tracking.gen.rs` from `lib.rs`

## 2. Tracking

- [ ] 2.1 `cli`: the profile snapshot and the permission mapping; the profile before the change read in the change's transaction
- [ ] 2.2 `grant` takes the tracker and tracks `profile_granted` when it creates a profile or adds a permission
- [ ] 2.3 `revoke` takes the tracker and tracks `profile_revoked` when it takes a permission away or removes the profile
- [ ] 2.4 The server's `admin` command builds its tracker (role `cli`); every other caller passes one

## 3. Tests

- [ ] 3.1 A test per event, through the CLI functions, reading it back from `kernel.tracked_events` (name, version, actor `system` `admin`, no workspace, no operation, snapshot, `before`), and that a no-op (a re-grant, a revoke of nothing) tracks nothing
- [ ] 3.2 An account id that is not a Campbooks id stays out of the snapshot
- [ ] 3.3 The server's coverage test lists nothing of `admin`

## 4. Last

- [ ] 4.1 OpenAPI regenerated (unchanged), client types unchanged
- [ ] 4.2 Tracking plan regenerated
- [ ] 4.3 Changelog entry file (`changelog/unreleased/added/331-track-admin.md`)
- [ ] 4.4 Gates green in CI
