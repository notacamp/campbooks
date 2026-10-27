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

## Deviations while building

Where the spec turned out to be wrong or short, the closest secure thing was
built. Each is listed here.

- **Four operations the spec did not name.** The screens need to read a
  table's sync and its conflicts, to stop it, and to undo a choice, so
  part 1 also has `pages.get_table_sync` (read; the table's `read` access),
  `pages.undo_resolve_table_conflict` and `pages.stop_table_sync` (the
  table's `write` access), next to `pages.resolve_table_conflict`.
- **How a database is imported.** `pages.import_from_notion` gains
  `data_sources` (Notion's id of each database and a title to show). With the
  flag off the answer is 404, as for a route that does not exist. Listing
  the databases the member shared (a Notion search for data sources) is part
  2's, with the dialog.
- **Confirmation is a query parameter.** `DELETE …/properties/{id}` has no
  body, so `confirm_remote_delete=true` is a query parameter. A column that is
  Notion's and a table that is syncing: without it, 400 `confirmation_required`
  and nothing changes. The confirmation is kept on the column's base
  (`remove_confirmed`), bound into the request and the connector's own MAC,
  and checked by the connector (`confirmation_required` again) before it
  calls Notion.
- **What Notion's API cannot do is not tried.** It cannot create a status
  column or change a status's groups, and it matches select options by id or
  name, renaming and reordering none and removing what a list omits. So a
  column of the kinds it can make is sent (rich text, number, select,
  multi-select, date, checkbox, URL, email, phone), a status column made here
  stays here, options are only ever added (never renamed, recoloured or
  removed from here), and the connector keeps every option Notion has by id.
- **Kinds that stay Notion's.** Formula, rollup, people, created-by,
  last-edited-by, files, relations, created and last-edited time, unique ID,
  button, verification and place come across as the reserved read-only types
  (`notion_formula`, `notion_rollup`, `notion_people`, `external`), with
  Notion's value as short text or names, refreshed each sync. A relation is
  `external` even when its database is synced too (relations between synced
  tables are a later step). Campbooks-only columns (a Campbooks link, people of
  the workspace, files, relations, created, edited, ID) are never sent.
- **The hashes.** The connector hashes each value and each column in its own
  form (SHA-256 of the canonical JSON of the neutral value); the pages module
  keeps both that hash (what Notion held) and the hash of its own form of the
  value (what was agreed). Neither side needs to know how the other writes a
  value.
- **A cell's outbound check is on the cell, not the row.** Notion gives one
  edit time per page, and a row edited in another cell is not a reason to hold
  a cell back (the "different cells" scenario), so a cell is written only
  when Notion's value for it still hashes as the base says; a column only when
  its description (or the whole schema) hashes as the base says; a row's trash
  only when the row's edit time is the one seen.
- **Reading.** Every run asks for the database (one call) and the entries
  edited at or after the cursor, a hundred a call, at most 1,000 entries a run
  (a run follows at once when more wait). An entry edited in the same minute as
  the cursor is not read again (the cursor remembers which). A first read, or
  one after the columns changed, reads every entry. Entries that went away
  are found by listing every id as titles alone, hourly and when the webhook
  woke the stream. More than 25 titles, texts or people in one value are read
  in full.
- **A change of kind at Notion replaces the values.** When Notion retypes a
  column the values here are kept for 30 days (`previous`, as for any retype,
  restorable) and Notion's take their place from the full read that follows.
- **Rows.** A row gone from Notion and unchanged here goes to the trash (30
  days); one changed here is kept and sent again as a new row. A row trashed
  here is moved to Notion's trash, never deleted, and only if the page is a row
  of this database that Notion has not edited since.
- **Row bodies.** Every row is linked to its Notion page, but only the first
  100 rows of a table get a stream of their own (a body syncs like a synced
  page): Notion's request budget is the connection's, so every row polling
  would starve it.
- **Requests.** At most 200 changes per request; the next ones follow when it
  ends. The data source's `PATCH` is only ever given `properties`, so a database
  can't be trashed or retitled from here.
- **Dates** always allow a time, as Notion's do not say if they have one.
- **Number formats.** Percent and six currencies (dollar, euro, pound, yen,
  Canadian and Australian dollar) map to their counterparts; every other
  format reads as a plain number.
- **The port.** The kernel's page-requests port gained `table_link_standing` and
  `stored_table_changes`, `pages.notion_link_requested` an optional `kind`
  (`page` by default), and the connectors reason enum
  `confirmation_required`; all additive.
- **Undoing a choice** puts the base and, for Notion's choice, our version
  back. For a column whose type Notion's choice changed, it uses the retype's
  `previous_*` as `pages.restore_property` does.
