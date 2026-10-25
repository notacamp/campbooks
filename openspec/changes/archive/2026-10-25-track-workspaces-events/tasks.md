# Tasks

## 1. The plan

- [ ] 1.1 `tracking/patches/0005-workspaces.yaml`: Changelog row (plan version 5), the 16 events, their properties, the records `workspaces.workspace`, `.member`, `.invitation`, `.signup_request`, `.signup_code`, their enums, a Coverage row for every mutating operation, handler and job of the module, and the conversations and identity exemptions renamed to `workspaces.workspace_deleted`
- [ ] 1.2 `apply`, `generate`, `check`; commit the workbook, the mirror and the generated code; include `tracking.gen.rs` from `lib.rs`

## 2. Tracking

- [ ] 2.1 `domain::tracked`: the snapshots (workspace, member, invitation, signup request, signup code) read from the module's rows, and the enum mappings
- [ ] 2.2 Workspaces: created, updated (a no-op tracks nothing), deleted, purged (the job)
- [ ] 2.3 Members: joined (creating, accepting), role changed (the same role tracks nothing), left or removed
- [ ] 2.4 Suspensions: announced in each workspace, only when the change is new for the user
- [ ] 2.5 Invitations: created, with the invitations it replaced
- [ ] 2.6 Signups: request filed, approved, rejected (a retry tracks nothing); code minted, revoked (a retry tracks nothing), redeemed

## 3. Tests

- [ ] 3.1 The module's test kit writes the outbox on demand and runs the admin routes in the request tracking context, as the server does
- [ ] 3.2 A test per live event reading it back from `kernel.tracked_events` (name, version, actor, workspace, operation, snapshot), that a no-op tracks nothing, and that no body holds an address or a name
- [ ] 3.3 The server's coverage test lists nothing of `workspaces`

## 4. Last

- [ ] 4.1 OpenAPI regenerated (unchanged), client types unchanged
- [ ] 4.2 Tracking plan regenerated
- [ ] 4.3 Changelog entry file (`changelog/unreleased/added/328-track-workspaces.md`)
- [ ] 4.4 Gates green in CI
