## Why

Pages sync with Notion (#680), and tables exist (#681, #684). Both are live
in prod. The owner's next step (2026-10-25) is for Notion **databases** to
come in as tables and stay in sync, so the clients, projects and content
plans members keep in Notion sit next to their mail.

Owner decisions:

- (2026-10-25) Import, then two-way. Only what the member shares in Notion's
  consent screen. Per member, private.
- (2026-10-27) **Columns sync both ways**, as well as values.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/377 (epic
#363). Prototype: board "Notion databases as tables" on
https://claude.ai/artifact/B4DKyoW5itjeAhcgAaY2rs

## Delivered in two changes

Following the owner's smaller-steps rule:

- **This change (part 1):** the server side, behind
  `CAMPBOOKS_CONNECTORS__NOTION__DATABASES__ENABLED`. It covers the connectors
  data-source stream and writes, the pages mapping of columns and rows, the
  conflicts, and the import operation, with tests against the fake Notion.
- **Part 2** (`notion-databases-ui`, its own PR):
  - databases in the import dialog;
  - the synced-table badge;
  - read-only Notion columns;
  - the column and cell conflict cards;
  - the "Remove from both places?" confirmation;
  - the recording.

Modules touched: **`connectors`** (the Notion provider gains data sources) and
**`pages`** (tables gain a Notion link). They talk through change streams and
outbound changes, as in #680. There's no new module dependency.

## What Changes

- **Import a database.** It becomes a private table of the member's. Its
  columns become table properties with the counterparts listed in
  `pages-tables` design.md, and each entry becomes a row page. A row's page
  body syncs like a synced page (#680).
- **Values sync both ways**, cell by cell. Per-cell versions on our side, and
  Notion's `last_edited_time` with a value hash on theirs, give a three-way
  compare. The same cell changed on both sides between syncs becomes a cell
  conflict the member resolves.
- **Columns sync both ways.**
  - Adding, renaming, retyping or removing a column, or changing its
    options, in either place reaches the other.
  - Campbooks-only types (Campbooks link) stay local and are marked so.
  - Notion formula, rollup, people and unsupported kinds come across as the
    read-only reserved types (`notion_formula`, `notion_rollup`,
    `notion_people`, `external`). Their values are Notion's, and are
    refreshed each sync.
  - A relation to a database that isn't synced is `external`.
- **Removing a column in Campbooks** removes it in Notion too, and the API
  requires an explicit confirmation flag. Campbooks keeps the column's values
  for 30 days (part 1's retention), so `pages.restore_property` recreates the
  column in both places and writes the values back.
- **Column conflicts:** the same column changed on both sides between syncs
  (rename versus rename, type versus type, options) is raised for the member
  to choose. A remove on one side with an edit on the other keeps the column.
  Nothing is deleted on a conflict.
- **Views stay local.** Notion keeps its own views, and Campbooks views are
  never sent.
- **Off by default** behind its own flag, which also needs Notion and tables
  on.

## Non-goals

- Syncing views, creating Notion formulas or rollups from Campbooks, and
  syncing database templates or page icons and covers.
- A database inline in a page (Notion's inline databases). Each comes across
  as a "Kept in Notion" block in v1.
- Exporting a Campbooks table that didn't come from Notion.
