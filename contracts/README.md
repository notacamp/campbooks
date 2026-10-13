# Contracts between modules

Modules never import each other (AGENTS.md). When one module produces a
document or event another consumes, and the consumer cannot read the
producer's Rust types, the shape lives here as a versioned JSON Schema
(draft 2020-12). Both sides keep a contract test that validates what they
produce or accept against the file.

| File | Producer | Consumer | What |
| --- | --- | --- | --- |
| `message-change-batch.v1.schema.json` | `connectors` | `conversations` | A batch of message changes for one inbox, stored as a blob (`application/vnd.campbooks.message-changes+json; version=1`) |
| `connectors.messages_changed.v1.schema.json` | `connectors` | `conversations` | The event announcing a batch (ids and a blob reference only) |
| `conversations.changes_requested.v1.schema.json` | `conversations` | `connectors` | A member's change the provider should mirror (outbound intent; ids and change kinds only) |
| `connectors.capability_granted.v1.schema.json` | `connectors` | `conversations` | The event announcing a connection's new capability; for `messages`, the inbox to create |
| `connection-account.v1.schema.json` | `connectors` | `conversations` | The account behind a connection (address, name), as the blob that event points at |
| `outgoing-message.v1.schema.json` | `conversations` | `connectors` | A message a member asked to send (addresses, subject, bodies, attachment blobs), as a blob (`application/vnd.campbooks.outgoing-message+json; version=1`) |
| `conversations.send_requested.v1.schema.json` | `conversations` | `connectors` | The event announcing a send once its undo window ends (ids and the outgoing message blob) |
| `connectors.send_completed.v1.schema.json` | `connectors` | `conversations` | The event telling how a send ended: sent (with the provider's id) or failed (with a reason) |
| `connectors.connection_removed.v1.schema.json` | `connectors` | `conversations` | The event announcing a disconnected connection, with the inboxes it fed and whether their mail is deleted or kept read-only |

Rules:

- **Never change a published version's meaning.** Adding an optional field
  is allowed when every consumer ignores unknown fields; anything else is a
  new file (`.v2.`) that both sides support during the switch.
- **No personal data in events.** Event payloads hold ids and blob
  references; names, addresses and subjects travel in blobs, which are
  deleted with the workspace.
- **Blobs are put before they are referenced.** The producer stores bodies,
  attachments and the batch in the workspace's blob store first, and keeps
  the batch attached to its own record until it no longer needs it; the
  consumer attaches what it keeps to its own records.
