# Tasks

## 1. The plan

- [ ] 1.1 `tracking/patches/0003-identity.yaml`: Changelog row (plan version 3), the 27 events, their properties, the records `identity.account`, `.session`, `.way_in`, `.passkey`, `.client`, `.consent`, their enums, and a Coverage row for every mutating operation, state-changing read, handler and schedule of the module
- [ ] 1.2 `apply`, `generate`, `check`; commit the workbook, the mirror and the generated code; include `tracking.gen.rs` from `lib.rs`

## 2. Tracking

- [ ] 2.1 `domain::tracked`: the snapshots (account, session, way in, passkey, client, consent) read from the module's rows, client ids kept only when Campbooks minted them, and the enum mappings
- [ ] 2.2 Signing in: provider callback, new account, passkey sign-in and sign-up (succeeded, failed, account created, counter regressed)
- [ ] 2.3 Sessions: logout (`session_ended`), sign out everywhere (`sessions_ended`)
- [ ] 2.4 Ways in: linked by a round trip, removed (by the person, a failed hand-over, prune), removed with the last connection (handler); the hand-over keeps the request's tracking context
- [ ] 2.5 Passkeys: registered, renamed (a no-op tracks nothing), removed
- [ ] 2.6 Admin: suspend and unsuspend (a retry tracks nothing)
- [ ] 2.7 Apps: workspace clients registered, rotated, removed; dynamic registration; consents granted, denied, revoked; a departed member's consents and apps; held grants discarded and adopted
- [ ] 2.8 Tokens: code exchange, reuse, revocation
- [ ] 2.9 The envelope's `client` actor carries a client id only when Campbooks minted it (`tracking::minted_client_id`, shared with the snapshots)

## 3. Tests

- [ ] 3.1 The module's test kit writes the outbox and runs the protocol and admin routes in the request tracking context, as the server does
- [ ] 3.2 A test per live event reading it back from `kernel.tracked_events` (name, version, actor, workspace, operation, snapshot), that a no-op tracks nothing, and that no body holds an address
- [ ] 3.3 The server's coverage test lists nothing of `identity`

## 4. Last

- [ ] 4.1 OpenAPI regenerated (unchanged), client types unchanged
- [ ] 4.2 Tracking plan regenerated
- [ ] 4.3 Changelog entry file (`changelog/unreleased/added/326-track-identity.md`)
- [ ] 4.4 Gates green in CI
