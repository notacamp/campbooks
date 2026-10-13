# Tasks

## 1. Scaffold and design

- [x] 1.1 `cargo xtask new-module conversations`: crate, scopes, schema migration, client module, i18n files, this change
- [x] 1.2 Proposal and design with alternatives; gate reviewed (constructor injection instead of a service map; no personal data in events)
- [x] 1.3 Translate the client module title (pt-PT, es-ES)

## 2. Contracts

- [x] 2.1 `contracts/message-change-batch.v1.schema.json`, `contracts/connectors.messages_changed.v1.schema.json`, `contracts/conversations.changes_requested.v1.schema.json`, README
- [x] 2.2 Contract tests: fixture batches and requested changes validated against the schemas

## 3. Kernel

- [x] 3.1 `JobHandler` / `EventConsumer` registered as values (`Background::handler`, `Background::consumer`), `jobs::enqueue_to`, with a test (the same change landed first with #36; this change keeps none of its own)
- [x] 3.2 The server's `Composition` (AI gateway, module settings) composes `conversations::module`; the `Visibility` port is registered

## 4. Behaviour

- [x] 4.1 Schema `conversations` with forced row-level security on every table, grants, list and threading indexes
- [x] 4.2 `apply_changes`: survey, prepare (participants document, rules), apply in order; threading with late-parent merges; blobs attached and detached; events
- [x] 4.3 Decide pass on `campbooks_ai::decide` (`EmailSignals`, `Model`, triage questions, envelope builder), per-question policy from the eval baseline with a test against `baseline.json`, `decisions` with reasons
- [x] 4.4 `decide_message`: model pass as suggestions, backfill window setting, idempotent, provider errors without content
- [x] 4.5 Operations: list inboxes, list / get conversations, message body, read / unread, star / unstar, archive / unarchive, move to stream (correction), labels add / remove; `changes_requested`
- [x] 4.6 Workspace purge on `workspaces.workspace_deleting`
- [x] 4.7 Recorded fixtures (en, pt-PT, es-ES, fr) and dev helpers; server message catalogue for the module's errors

## 5. Follow-ups

- [x] 5.1 k-NN over the workspace's corrections and a cascade: moved to campbooks-cloud#83
- [x] 5.2 Consume `connectors.messages_changed` (enqueue `apply_changes`, batch held) and `connectors.capability_granted` (create the inbox, grantor as manager), now that #36 publishes them
- [x] 5.4 Change feed: conversation and message changes declared within their inbox; the `Visibility` port answers by inbox membership
- [x] 5.5 Mounting under the workspace route of campbooks-cloud#451: whichever of #451 and this change lands second does it
- [x] 5.3 Custom streams and inbox sharing: moved to campbooks-cloud#84; `MessageDecided` / `AttachmentReceived`: moved to campbooks-cloud#85; the module scaffold's client template: campbooks-cloud#72

## 6. Wrap-up

- [x] 6.1 OpenAPI and events catalogue regenerated; client types regenerated
- [x] 6.2 CHANGELOG updated
- [x] 6.3 Gates green
- [x] 6.4 No UI change beyond the scaffold's placeholder page (no recording)
