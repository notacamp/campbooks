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
| `conversations.need_changed.v1.schema.json` | `conversations` | `attention` | What a conversation needs from its inbox's members, opened, changed or cleared (ids, enums and a date only) |
| `conversations.member_replied.v1.schema.json` | `conversations` | `tasks` | The member sent a message in a conversation of their personal inbox: asks "Done?" of their open Do tasks that came from it (ids and a time only) |
| `tasks.task_changed.v1.schema.json` | `tasks` | `attention` | A task made or changed: state, verb, due, plan, assignee, source and whether it asks "Done?" (ids, enums, flags, dates and times only, never a title) |
| `pages.page_changed.v1.schema.json` | `pages` | `search` | A page made or changed, trashed, moved, shared or deleted (ids and enums only, never a title or any text) |
| `pages.table_changed.v1.schema.json` | `pages` | the change feed | A table's properties, views or set of rows changed (ids and enums only, never a name or a value) |
| `connectors.send_completed.v1.schema.json` | `connectors` | `conversations` | The event telling how a send ended: sent (with the provider's id) or failed (with a reason) |
| `connectors.connection_removed.v1.schema.json` | `connectors` | `conversations` | The event announcing a disconnected connection, with the inboxes it fed and whether their mail is deleted or kept read-only |
| `conversations.message_read.v1.schema.json` | `conversations` | `schedule`, `tasks` | The event announcing a stored read of one message: its items (things at a time), its asks (obligations) and whether a summary is ready (ids, enums, keys and times only) |
| `read-texts.v1.schema.json` | `conversations` | `schedule`, `tasks` | The words of a stored read, as the blob `conversations.message_read` points at in `texts` (`application/vnd.campbooks.read-texts+json; version=1`): item titles, places and calendar UIDs, and the titles and amounts of the asks |
| `calendar-list.v1.schema.json` | `connectors` | `schedule` | The calendars of a connection's account at the provider, as the blob `connectors.capability_granted` points at for the `calendar` capability (`application/vnd.campbooks.calendar-list+json; version=1`) |
| `connectors.calendar_changed.v1.schema.json` | `connectors` | `schedule` | The event announcing a batch of changes to one provider calendar (ids and a blob reference only) |
| `calendar-change-batch.v1.schema.json` | `connectors` | `schedule` | A batch of events upserted and removed on one provider calendar, as the blob that event points at (`application/vnd.campbooks.calendar-changes+json; version=1`) |
| `schedule.event_created.v1.schema.json`, `schedule.event_changed.v1.schema.json`, `schedule.event_cancelled.v1.schema.json` | `schedule` | (any) | An event added to, changed on or cancelled from a calendar (ids, enums and times only; no title, place or attendees) |
| `schedule.invitation_changed.v1.schema.json` | `schedule` | `attention` | An invitation awaiting the member's answer opened, changed or closed, with a revision (ids, a day, a time and flags only; no title, place or organiser) |
| `schedule.outbound_requested.v1.schema.json` | `schedule` | `connectors` | A member's change to an event on a provider calendar, to be made there once schedule confirms it (ids, the kind of change and a blob reference) |
| `calendar-change.v1.schema.json` | `schedule` | `connectors` | What to write at the provider and against which etag, as the blob that event points at (`application/vnd.campbooks.calendar-change+json; version=1`) |
| `schedule.calendar_sync_changed.v1.schema.json` | `schedule` | `connectors` | A member turned a provider calendar on or off (ids, a flag and a blob reference to which calendar of the provider it is) |
| `calendar-ref.v1.schema.json` | `schedule` | `connectors` | Which calendar of the provider a sync change is about, as the blob that event points at (`application/vnd.campbooks.calendar-ref+json; version=1`) |
| `connectors.calendar_change_completed.v1.schema.json` | `connectors` | `schedule` | The event telling how a calendar change ended: applied (with the provider's id and etag), superseded, skipped or failed (with a reason code) |
| `notion-page-snapshot.v1.schema.json` | `connectors` | `pages` | A Notion page's blocks as the connector read them (neutral kinds, rich text, files already downloaded), as a blob (`application/vnd.campbooks.notion-page-snapshot+json; version=1`) |
| `connectors.notion_page_read.v1.schema.json` | `connectors` | `pages` | The event announcing a snapshot (ids, a blob reference and a time only) |
| `connectors.notion_link_stopped.v1.schema.json` | `connectors` | `pages` | The event telling that a Notion page is no longer read (not shared, or trashed there), so the pages module ends the link |
| `notion-block-changes.v1.schema.json` | `pages` | `connectors` | The blocks to write to a Notion page (updates, appends, deletes), as a blob (`application/vnd.campbooks.notion-block-changes+json; version=1`) |
| `pages.notion_changes_requested.v1.schema.json` | `pages` | `connectors` | A member's edits asked for at Notion, once the pages module stored and bound them (ids and a blob reference only; confirmed back through the kernel's page-requests port) |
| `connectors.notion_changes_completed.v1.schema.json` | `connectors` | `pages` | The event telling how each change of a request ended: applied (with the block's id and edit time at Notion), skipped or failed (with a reason code) |
| `pages.notion_link_requested.v1.schema.json` | `pages` | `connectors` | A page was linked to a Notion page, which now needs a stream (ids only; the link is confirmed through the page-requests port) |
| `pages.notion_link_ended.v1.schema.json` | `pages` | `connectors` | A page stopped being kept in step with its Notion page, so its stream goes (ids only) |
| `notion-data-source-snapshot.v1.schema.json` | `connectors` | `pages` | A Notion database (data source) as the connector read it: its properties, the entries edited since the last read with each cell's neutral value and hash, and on a sweep the ids of every live entry, as a blob (`application/vnd.campbooks.notion-data-source-snapshot+json; version=1`) |
| `connectors.notion_data_source_read.v1.schema.json` | `connectors` | `pages` | The event announcing a database snapshot (ids, a blob reference and a time only) |
| `notion-table-changes.v1.schema.json` | `pages` | `connectors` | The columns, rows and cells to write to a Notion database, as a blob (`application/vnd.campbooks.notion-table-changes+json; version=1`) |
| `pages.notion_table_changes_requested.v1.schema.json` | `pages` | `connectors` | A member's table edits asked for at Notion, once the pages module stored and bound them (ids and a blob reference only; confirmed back through the page-requests port; a column removal only with the member's confirmation) |
| `connectors.notion_table_changes_completed.v1.schema.json` | `connectors` | `pages` | The event telling how each table change of a request ended: applied (with the column's or row's id, edit time and a hash), skipped or failed (with a reason code) |

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
