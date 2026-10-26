## Why

Part 1 of Tables (`pages-tables`, notacamp/campbooks#681) shipped the model and
API behind `CAMPBOOKS_PAGES__TABLES__ENABLED`: typed properties, per-cell values,
table and board views, and the query. It has no screens. This is part 2: the
screens the owner approved in the prototype, so tables can be switched on.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/365 (epic
#363). Prototype: board "Tables" on
https://claude.ai/artifact/B4DKyoW5itjeAhcgAaY2rs. Owner decisions (Table and
Board in v1, no formula or rollup editor) are unchanged.

Modules touched:

- **`pages`**, small server changes:
  - `kind` (`page` | `table`) on `PageSummary` and `PageView`, so the tree and
    the screen know a table (part 1 deviation);
  - the `tableView` block (`table`, `view`) added to the block schema, for
    inline tables in a page.
- **Client**, in the pages module:
  - the table page, with table and board views;
  - cell editors;
  - the row peek;
  - the new-column menu;
  - filters and sorts;
  - the inline table blocks.

No new module and no new dependency between modules.

## What Changes

- **A table page:**
  - view tabs and "+ view";
  - a one-row toolbar (filter chips, Filter, Sort, New);
  - a grid with a header per property type and a column menu (rename,
    type, hide, sort, filter, delete);
  - cell editors for every type;
  - keys: arrows, Enter, Esc, ⌘↵, and `n r` for a new row;
  - footer summaries.
- **Board:** a column per group with its count and summary, and drag
  between groups (with undo).
- **Row peek:** a side panel with the row's properties and its page body,
  plus "Open as page".
- **New column** from a type menu grouped Basic, Links, and Filled in for you.
  The Notion-only read-only types are never offered.
- **Inline tables:** `/table` makes an inline table in a page, and
  `/link to table` embeds a view of an existing one. A reader without access
  to the table sees "Something you can't open".
- **The tree** shows tables with a table icon. "New table" sits beside
  "New page".
- **Phone:** the grid and board scroll inside their own box, and the row peek
  opens full screen.

## Non-goals

As in part 1:

- formula and rollup editors;
- list, calendar and gallery views;
- CSV import and export;
- Notion database sync (cloud #377).
