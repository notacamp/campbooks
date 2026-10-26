# Design

## Alternatives considered

**Where tables live.**

- *Inside `pages`, rows as pages* (chosen). Notion's model is the owner's
  model: a database row is a page with properties and a body. In `pages`, a
  row gets the page tree, access, trash, search, mentions and backlinks for
  free, and Notion sync (#680) already links pages to Notion pages, so a
  Notion row maps to a row page. The property types are written as a
  self-contained library (`pages::properties`: types, configs, validation,
  comparison and SQL casting) with no dependency on the rest of the module.
  The `records` module of ADR 0008 can adopt it later. ADR 0022 records this
  amendment.
- *The `records` platform module first* (ADR 0008's plan). Rejected for now:
  it is a much larger build (custom types for every module, a generic view
  engine), and rows that are pages would cross two modules on every read.
- *A Yjs document per table.* Rejected: filtering, sorting and summaries over
  thousands of rows need SQL, and per-cell versions already stop concurrent
  edits to different cells from colliding.

**How values are stored.**

- *One row per cell* (chosen). `pages.row_values (row_id, property_id,
  value jsonb, version, updated_by, updated_at)`. The value is validated
  against its property's type, every cell has its own version, and the query
  casts by type.
- *One JSONB document per row.* Rejected: concurrent edits to two cells of
  the same row would need merging, and an index per property is harder.

## Data (schema `pages`)

- `pages.pages.kind`: `page` (default) | `table` | `row`. A row's parent is
  its table, and its title is the table's title property.
- `pages.table_properties`: `id` (uuid v7), `table_id`, `name` (1–100
  characters), `type`, `config` jsonb (validated per type), `position`,
  `created_by`. At most 200 per table, and exactly one `title`.
- `pages.row_values`: the primary key is (`row_id`, `property_id`), with
  value jsonb, `version` bigint, `updated_by` and `updated_at`. Created,
  edited, ID and title are not stored here: they come from the page row, or
  from `unique_id` on the row page (a per-table sequence).
- `pages.table_views`: `id`, `table_id`, `name`, `type` (`table` | `board`;
  `list`, `calendar`, `gallery` reserved), `position` and `config` jsonb:
  - `filter`: a tree of AND/OR groups (depth 3), each condition being
    property, operator and value, with operators per type;
  - `sorts`: property and direction, up to 5;
  - `properties`: visible, order and width;
  - `summaries`: property → count, sum, average, min, max, percent checked;
  - for a board: `group_by` (a select or status property), hidden groups and
    card properties.
- Limits: 20,000 rows per table, 100 options per select, value JSON up to
  16 KiB, text up to 2,000 characters, and up to 100 related rows per
  relation cell.

## Property types and their Notion counterparts

| Ours | Config | Value | Notion |
| --- | --- | --- | --- |
| `title` | — | (page title) | `title` |
| `text` | — | rich-text runs (marks, links: https/http/mailto) | `rich_text` |
| `number` | `format`: number, percent, currency (ISO code) | decimal string | `number` (a format we lack becomes `number`) |
| `select` | `options` [{id, name, color}] | option id | `select` (option ids mapped) |
| `multi_select` | `options` | option ids | `multi_select` |
| `status` | `options` + `groups` (to-do, doing, done) | option id | `status` |
| `date` | `time`: bool | {start, end?, time_zone?} | `date` |
| `checkbox` | — | bool | `checkbox` |
| `person` | — | member ids | `people` → `notion_people` (read-only names; Notion users aren't mapped to members) |
| `files` | — | blob refs (name, size, type) | `files` |
| `url` / `email` / `phone` | — | string (checked shape) | `url` / `email` / `phone_number` |
| `relation` | `table`, `pair` (property id or none) | row ids | `relation` (to a synced data source; else `external`) |
| `campbooks_link` | allowed ref types | RecordRefs | — (sent to Notion as "Campbooks link" text) |
| `created` / `edited` | `of`: time or who | computed | `created_time` / `created_by` / `last_edited_*` |
| `unique_id` | `prefix` | computed | `unique_id` |
| `notion_formula`, `notion_rollup` | source type | read-only snapshot | `formula`, `rollup` (step 3) |
| `external` | Notion kind label | none | `verification`, `button`, `place`, … (kept in Notion) |

A paired relation updates both sides in one transaction. Deleting a related
row removes it from relation cells. A Campbooks link cell renders each
reference through its own module with the viewer's access, exactly like a
mention, and never stores a subject.

## Operations

All need `pages:read` or `pages:write`, are checked through the page's
`authz` (the table page for schema and views, the row page for its values),
carry undo, follow idempotency, and are rate-limited:

| Operation | Does |
| --- | --- |
| `pages.create_table` | A table page with a title property and a default table view. |
| `pages.add_property` / `update_property` / `remove_property` | Schema changes. Changing a type converts the values it can and keeps the rest as text (undo restores the old ones). Removing keeps values for undo for 30 days. |
| `pages.set_options` | Add, rename, recolour and reorder options, by id. |
| `pages.add_row` / `pages.set_cells` | A row (a page of kind `row`) and cell writes, with the version of each cell: a stale version still writes, and answers the replaced value for the undo toast. |
| `pages.query_table` | Rows for a view or ad-hoc filter and sort, 100 per page with a cursor, plus summaries. Executed in SQL with typed casts. Only rows the caller may read. |
| `pages.create_view` / `update_view` / `remove_view` | The views (at least one stays). |

Rows are moved, trashed and restored with the existing page operations.

## Events, search, tracking

- `pages.page_changed.v1` gains the kinds `schema`, `values` and `views`.
  These carry ids only, never values.
- `pages.table_changed.v1` (`table_id`, `revision`, kinds) lets open views
  refresh through the change feed.
- Search: row titles and `text` values, through the pages `SearchSource`.
- Tracked events (ids, enums and counts only):
  - `table_created`;
  - `property_added` (type);
  - `view_created` (type);
  - `rows_queried` (rows bucket, view type);
  - `cells_edited` (count bucket).
  Coverage rows for every operation.

## Client

- **A table page:**
  - view tabs and "+ view";
  - a toolbar that stays one row (filter chips, Filter, Sort, New);
  - the grid with a header for each property type and a column menu
    (rename, type, hide, sort, filter, delete);
  - cell editors per type;
  - keys: arrows move, Enter edits, Esc cancels, ⌘↵ opens the row, and
    `n r` adds a new row;
  - footer summaries.
- **Board:** columns by group, drag between groups (with undo), and card
  properties.
- **Row peek:** a side panel with the properties and the row's page body,
  plus "Open as page".
- **Slash menu:** `/table` makes a new inline table, and `/link to table`
  embeds a view of an existing one.
- **Phone:** the grid scrolls inside its box, and the board scrolls by
  column. Must pass `pnpm test:fit` at 375 and 1280 in every locale.

## The flag

`CAMPBOOKS_PAGES__TABLES__ENABLED` is off by default. When off, there are no
table routes, no slash items, and no table page kind in lists. Stray settings
never stop the server.

## Deviations while building

- **Property-based tests use a seeded generator**, not the `proptest` crate: no new
  dependency (lockfile, `cargo deny`) for what a 20-line xorshift does, and a failure replays.
  The SQL is held to an in-memory oracle (`properties::eval`) on random tables in a real database.
- **Status groups are a field of each option** (`group`: `todo`, `doing`, `done`), not a
  separate `groups` list in the config.
- **`kind` is not on `PageSummary`/`PageView`** in part 1: it would be a required field and
  change every client fixture. Part 2 adds it. Until then tables are listed as ordinary pages
  (and only while the flag is on); rows are never listed.
- **Operations beyond the table:** `pages.get_table` (schema and views) and
  `pages.restore_property` (undo of a removal or of the last change of type). The undo of
  `pages.update_property` is itself for a rename; a change of type is taken back with
  `restore_property` (`can_revert` says when).
- **Type change** clears the cells that do not convert and keeps what every changed cell held
  (`row_values.previous`, 30 days) instead of "keeping the rest as text".
- **The cursor** of `query_table` is an offset (`o200`), not a keyset: with sorts on any type and
  20,000 rows it is exact and cheap, and a row added between pages can shift one. Rows past it
  are not skipped when sorts are stable.
- **No sub-pages under a row or a table** in part 1 (`table_parent`), so a row's access is its
  table's and the page-access walk skips the 20,000 rows. Rows are left out of `pages.list`,
  `roles::readable` and backlinks.
- **A row's container is its table** for search and the change feed (`PageChanged.row`), so the
  index and the feed see one container per table.
- **`pages.table_changed`** is published for schema, views and the set of rows; a cell edit is
  `pages.page_changed` of its row (kind `values`), so editing cells never locks the table.
- **A paired relation** is kept in step while both properties are live; a removed pair makes the
  relation one-way until it is restored. Removing a property never edits another table.
- **Retention:** removed properties and type-change backups are swept after 30 days by the daily
  `pages.purge_trash` (its dispatcher reads them through a maintenance policy).
- **Not done in part 1:** reindexing rows whose property was converted to or from text in bulk
  (they reindex at their next edit), and scrubbing a left member's id from person cells.
