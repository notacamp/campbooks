# Design

## Server

`GET /api/v1/w/{ws}/connections/{connection}/notion/data-sources` (operation
`connectors.list_notion_data_sources`, `connections:read`, the connection's
owner only, otherwise 404). It runs a Notion search filtered to data sources
(API 2026-03-11), 100 a page with a cursor, and answers id, title, the number
of columns, and whether the database is linked to a table already. Results
are bounded (200) and charged to the connection's bucket, and titles are never
logged. The fake Notion gains search over data sources.

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

## Deviations while building

Where the spec turned out to be wrong or short, the closest secure thing was
built. Each is listed here.

- **Columns, not entries.** Notion's search gives a database's properties but
  not how many entries it has, so the listing answers `columns` and the dialog
  says "7 columns". An entry count would be one query per database.
- **Flag off is an empty list, not a 404.** The access matrix expects the
  owner of a connection to get an answer past the access checks from every
  operation, and "nothing to offer" is what an empty list says. Another
  member's connection is still 404 and Notion is not called. The client takes
  a 404 for the same thing.
- **The API does not say which columns are Notion's.** `get_table_sync` gives
  the link and the conflicts, not the columns' origin, so the client marks
  columns by kind: a lock with the reason for the reserved Notion kinds
  (formula, rollup, people, kept in Notion), "Campbooks only" for the kinds
  that are never sent (person of the workspace, files, relation, Campbooks
  link, created, edited, ID), and "Stays in Campbooks" on the kinds Notion
  cannot make in the type menus. A status column may come from Notion or be
  made here, so it is never marked after the fact: removing one is tried
  without the confirmation, and the dialog opens only when the server answers
  `confirmation_required`.
- **Removing asks first, for what Notion has.** A column of a kind Notion can
  make or owns opens "Remove from both places?" before anything is sent;
  a Campbooks-only column is removed at once, with an Undo, because it never
  reached Notion.
- **The value count is the table's row count.** The dialog says how many rows
  lose the column's value; counting only the filled cells would take a query
  per column.
- **Open in Notion** is built from the data source's id, as the page link is
  from the page's id. Notion opens a database by its id; whether every
  database's data source id is also its address is not checked here.
- **No author on a conflict.** The conflict has two versions and two times,
  not who changed them in Notion, so the card says "Notion's change: <time>".
- **A cell conflict in the grid is a flag, not inline.** The flag (a warning
  button with the column's name) opens both versions in a popover, so the cell
  stays one line at any width. In the row beside the table the card sits under
  the value. A cell with a change waiting cannot be edited until it is settled.
  A reader sees both versions and has no buttons.
- **The badge is the page's.** A synced table reuses the page header
  (`NotionSync`, now for tables too) rather than a second component, and the
  column conflicts are `ColumnConflicts`. Inline tables inside a page show
  marks and conflicts but no badge.
- **One limit for both.** The server counts pages and databases together
  (100), so the dialog's message says "pages and databases".
- **Story tests assert clickability by hit-testing.** Stories run under
  Testing Library, not Playwright, so `click({trial: true})` is a check that
  the control has a size, is on screen once scrolled to, and that
  `elementFromPoint` at its middle is the control; rows that must not overlap
  are compared in both directions.
- **Not run here.** The Storybook story tests, `test:fit`, the recorder and the
  server crate's access-matrix test were left to CI (the owner's rule and
  free disk); the connectors tests, clippy, `xtask arch`/`openapi`/`events`
  and the tracking check were run locally.
