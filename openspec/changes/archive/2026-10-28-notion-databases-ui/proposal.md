## Why

Part 1 (`notion-databases`, notacamp/campbooks#685) shipped the server side of
Notion databases as tables, behind `CAMPBOOKS_CONNECTORS__NOTION__DATABASES__ENABLED`:
import, two-way values and columns, conflicts, confirmed removal. There are no
screens yet. This is part 2, the screens from the approved prototype (board
"Notion databases as tables" on https://claude.ai/artifact/B4DKyoW5itjeAhcgAaY2rs),
so the flag can be switched on.

Tracking: https://github.com/notacamp/campbooks-cloud/issues/377 (epic #363).
Owner decisions are unchanged (import then two-way, picked only, per member;
columns both ways, 2026-10-27).

Modules touched:

- **`connectors`:** one read operation, `connectors.list_notion_data_sources`,
  for the import dialog. It searches the member's Notion for the databases
  they shared, using the same request budget and caps.
- **Client:** the pages module, with the import dialog, the synced-table
  header, Notion column marks, the conflict cards, and the remove
  confirmation.

## What Changes

- **Import from Notion** lists shared databases (title, entry count when
  Notion gives it) next to pages. Ticking one imports it as a table.
- **A synced table's header** shows "Synced with Notion · <time>", with Open
  in Notion, Stop syncing, and "N changes to look at" when there are
  conflicts.
- **Column marks:**
  - read-only Notion columns (formula, rollup, people, unsupported) show a
    lock with the reason, and their cells are not editable;
  - Campbooks-only columns show "Campbooks only";
  - a status column or option change made here says "Stays in Campbooks:
    Notion can't change this".
- **Conflict cards:** for a cell (Keep mine / Use Notion's) in the row peek
  and in the grid, and for a column (rename, type, options) in a banner above
  the table. Each has Undo, through part 1's resolve and undo operations.
- **Removing a synced column** opens "Remove from both places?". It names the
  value count and the 30-day restore, and sends `confirm_remote_delete=true`
  only on confirm.
- **Phone:** the cards and dialog fit 375 px in every locale.

## Non-goals

Syncing views, relations between synced tables, formula editing, and Notion's
inline databases. These are as in part 1.
