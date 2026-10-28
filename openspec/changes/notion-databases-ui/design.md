# Design

## Server

`GET /api/v1/w/{ws}/connections/{connection}/notion/data-sources` (operation
`connectors.list_notion_data_sources`, `connections:read`, the connection's
owner only, otherwise 404). It runs a Notion search filtered to data sources
(API 2026-03-11), 100 a page with a cursor, and answers id, title, and the
entry count when Notion gives it. Results are bounded and charged to the
connection's bucket, and titles are never logged. The fake Notion gains
search over data sources.

## Client

Part 1's operations do the work:

- `pages.get_table_sync` reads the sync state and the conflicts;
- `pages.resolve_table_conflict` and `pages.undo_resolve_table_conflict`
  answer a conflict and take it back;
- `pages.stop_table_sync` stops the sync;
- `pages.import_from_notion` with `data_sources` does the import;
- `DELETE …/properties/{id}?confirm_remote_delete=true` removes a synced
  column.

The screens:

- **ImportFromNotion** gains a "Databases" group (table icon, entry count),
  and its count toast includes tables.
- **The table page header** gains a `NotionSyncBadge`, plus a
  `TableConflictsBanner` for column conflicts.
- **Grid and peek:** cells of read-only kinds are view-only with a lock and
  a tooltip. A cell conflict shows Notion's value under ours, with the two
  choices.
- **The column menu** of a synced table:
  - Rename and Change type (only kinds Notion can make; others say they stay
    in Campbooks);
  - options are add-only, and rename, recolour or remove say "Stays in
    Campbooks";
  - "Remove from both places…" opens the confirm dialog.
- **Accessibility:** cards are groups with labels, actions are buttons, and
  the lock icon has an aria-label with the reason.
- **i18n:** en, pt-PT, es-ES and fr. `pnpm test:fit` passes at 375 and 1280.
- **Recording:** idle stories with a stateful fake, recorded at 375 px and
  desktop.

## Numbers and flag

This is the same flag (`CAMPBOOKS_CONNECTORS__NOTION__DATABASES__ENABLED`),
and no migration is expected. If one is needed, it uses the number the merge
coordinator reserves.
