# Design

Builds on #680 (`pages-notion-sync`: connection, stream, hook, MAC-bound
outbound, block mapping) and #681 (`pages-tables`: properties library, values,
retention).

## Notion API (version 2026-03-11, pinned)

- **Read:** `GET /v1/data_sources/{id}` for the schema (properties with ids,
  types, options). `POST /v1/data_sources/{id}/query` lists entries (pages)
  with their property values, 100 a page, sorted by `last_edited_time`, and
  filtered by `last_edited_time` after the cursor for incremental runs. A
  relation or people value over 25 items is read with
  `GET /v1/pages/{id}/properties/{property_id}`.
- **Write:**
  - `PATCH /v1/data_sources/{id}` for the schema: add, rename, change type,
    options, remove;
  - `POST /v1/pages` with `parent: {data_source_id}` for a new row;
  - `PATCH /v1/pages/{id}` for property values;
  - `in_trash` for a row deleted in Campbooks, which is the only kind of
    trashing allowed here.
- **Webhook events** added to the subscription by the owner:
  `data_source.content_updated`, `data_source.schema_updated`. Each only marks
  that data source's stream due. Without them, polling runs every 5 minutes.
- **Limits:** the same `notion:{connection}` bucket and the same caps. A first
  import of a large database queues within the bucket.

## Connectors

- **Stream `notion.data_source`,** one per linked database. The cursor is the
  newest entry `last_edited_time` plus a schema hash.
  - A run reads the schema when the hash differs, then the entries changed
    since the cursor.
  - It emits a `NotionDataSourceSnapshot`: properties (id, name, kind,
    options), and changed entries with each property value normalised and
    hashed.
  - Entry bodies are linked as page streams (#680), so a row's body syncs as
    a page.
- **Outbound kinds:** `notion.schema_change` (add, rename, retype, options,
  remove; remove carries `confirmed: true`), `notion.row_create`,
  `notion.cells_update`, `notion.row_trash`. All are MAC-bound like #680's,
  and checked against the base edit time and schema hash before writing.
  A held change becomes a conflict on the next inbound run.

## Pages

- **`pages.external_tables`:** `table_id`, `connection_id`,
  `notion_data_source_id`, `state`, `stopped_reason`, `created_by`.
- **`pages.external_properties`:** `property_id`, `notion_property_id`,
  `base_name`, `base_type_hash`, `base_options_hash`.
- **`pages.external_cells`:** `row_id`, `property_id`, `base_hash`,
  `base_edited_at`.
- Rows map through #680's `external_links` (row page ↔ Notion page).
- **Import** (`pages.import_from_notion` gains data sources): the database
  becomes a private table (rows, properties, a default table view), linked.
- **Apply a snapshot** in one transaction:
  1. Schema three-way per column against the base. Theirs changed alone: apply
     it through the properties library (a type change keeps previous values,
     part 1). Ours changed alone: leave it for outbound. Both changed: raise a
     column conflict. Remove on one side and edit on the other: keep the
     column and re-push the edit. A column new on their side is added; one
     gone on their side and unchanged on ours is removed, with values kept
     for 30 days for restore.
  2. Cells three-way per cell, the same way. Conflicts become cell conflicts.
  3. Read-only kinds are always overwritten with Notion's value.
- **Outbound** (debounced 10 s): diffs schema and cells against their bases
  and records outbound changes. It never sends Campbooks-only columns,
  read-only kinds or unresolved conflicts. Removing a column requires
  `confirm_remote_delete: true` on `pages.remove_property` for a synced table
  (400 `confirmation_required` otherwise). The UI's dialog sets it.
  `pages.restore_property` on a synced table recreates the Notion property
  and re-sends the kept values.
- **Conflicts:** `pages.table_conflicts` holds the kind (`column` | `cell`),
  ours, theirs, and when. `pages.resolve_table_conflict { conflict, keep:
  ours | theirs }` moves the base and lets the change flow, with undo.
- **Stopping:** stop, disconnect, trash, un-share or member left all stop the
  link, as for pages. Nothing is deleted in Notion.

## Access and security

- Everything goes through part 1's table access: the table for schema, the
  row for values. Import needs the caller's own Notion connection.
- No values, names or option names go into events, logs or job payloads.
  Hashes and ids only.
- A Campbooks link column is never sent. Mention-like Notion values (people)
  come in as display names only, read-only.
- The webhook lookup across workspaces only marks streams due, as in #680.

## Numbers and flag

Tracking patch `0019`, migrations from `20261112010000`, as reserved by the
merge coordinator. The flag `CAMPBOOKS_CONNECTORS__NOTION__DATABASES__ENABLED`
is off by default. It is effective only with Notion and tables on. When off,
no data-source routes, hook events or jobs exist, and stray settings never
stop the server (tested against production values).
