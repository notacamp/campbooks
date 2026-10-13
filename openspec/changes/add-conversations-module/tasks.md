# Tasks

## 1. Scaffold and design

- [x] 1.1 `cargo xtask new-module conversations`: crate, scopes, schema migration, client module, i18n files, this change
- [x] 1.2 Proposal and design with alternatives; gate reviewed (constructor injection instead of a service map; no personal data in events)
- [x] 1.3 Translate the client module title (pt-PT, es-ES)

## 2. Contracts

- [x] 2.1 `contracts/message-change-batch.v1.schema.json`, `contracts/connectors.messages_changed.v1.schema.json`, `contracts/conversations.changes_requested.v1.schema.json`, README
- [x] 2.2 Contract tests: fixture batches and requested changes validated against the schemas

## 3. Kernel

- [x] 3.1 `JobHandler` / `EventConsumer` registered as values (`Background::handler`, `Background::consumer`), `jobs::enqueue_to`, with a test
- [x] 3.2 Server `Deps` (AI gateway, module settings) composed into `conversations::module`

## 4. Behaviour

- [x] 4.1 Schema `conversations` with forced row-level security on every table, grants, list and threading indexes
- [x] 4.2 `apply_changes`: survey, prepare (participants document, rules), apply in order; threading with late-parent merges; blobs attached and detached; events
- [x] 4.3 Decide seam: `Envelope`, `Strategy`, `NoRules`, `ModelStrategy`, per-question switch, `decisions`
- [x] 4.4 `decide_message`: model pass as suggestions, backfill window setting, idempotent, provider errors without content
- [x] 4.5 Operations: list inboxes, list / get conversations, message body, read / unread, star / unstar, archive / unarchive, move to stream (correction), labels add / remove; `changes_requested`
- [x] 4.6 Workspace purge on `workspaces.workspace_deleting`
- [x] 4.7 Recorded fixtures (en, pt-PT, es-ES, fr) and dev helpers; server message catalogue for the module's errors

## 5. Follow-ups (not in this change)

- [ ] 5.1 Swap the decide seam for campbooks-cloud#59's `campbooks_ai::decide` strategies and rules
- [ ] 5.2 Consume `connectors.messages_changed` and `connectors.capability_granted` once #36 publishes them
- [ ] 5.3 Custom streams, inbox sharing operations, `MessageDecided` / `AttachmentReceived`

## 6. Wrap-up

- [x] 6.1 OpenAPI and events catalogue regenerated; client types regenerated
- [x] 6.2 CHANGELOG updated
- [x] 6.3 Gates green
- [x] 6.4 No UI change beyond the scaffold's placeholder page (no recording)
