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
