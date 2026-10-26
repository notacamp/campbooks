# Design

## Builds on part 1

Everything reads and writes through part 1's operations:
- `pages.get_table`;
- `pages.query_table`;
- `pages.add_property`, `update_property`, `remove_property`,
  `restore_property`, `set_options`;
- `pages.add_row`, `pages.set_cells`;
- `pages.create_view`, `update_view`, `remove_view`.

Part 1's deviations hold:
- an offset cursor;
- no sub-pages under a table or row;
- status groups as a field of each option;
- a type change keeps the cells' previous values for 30 days.

## Server changes (small)

- **`kind` on pages:** `PageSummary` and `PageView` gain `kind` (`page` |
  `table`). Rows are never listed. Client fixtures are updated with it. It is
  a required field, so OpenAPI and the client types are regenerated.
- **`tableView` block:** added to the block schema allowlist, with attributes
  `table` (page id) and `view` (view id). It is checked like other blocks (ids
  only; the server never copies the table's content into the document).
  Rendering it calls `query_table` with the viewer's access, and a 404 shows
  as "Something you can't open". `pages.links` records it as kind `embed` of
  `pages.page`, so backlinks show the table embedded where it is.

## Client

- **Components** (in `@notacamp/ui` where generic, with stories and tests):
  - `DataGrid`: virtualised rows; sticky header and first column; keyboard
    cell navigation with roving focus; column resize; an `overflow-x` box.
  - `CellEditor` per type: text, number, select and multi-select (option
    picker with create, rename and colour), status, date with range and
    time, checkbox, person (members by name), files (upload through the
    page's file op), URL, email, phone, relation (row picker by title,
    readable rows only), and Campbooks link (the `@` search by name).
  - `BoardView`: columns with drag between them (keyboard-accessible: a
    "Move to" menu as well).
  - `RowPeek`: a side panel, full screen on phones.
  - `FilterBuilder` and `SortBuilder` (the operators come from the property
    type).
  - `PropertyTypeMenu`.
- **Data:**
  - TanStack Query over `query_table` (pages of 100, offset cursor) and
    `get_table`.
  - Cell writes are optimistic, with the version check. A replaced value
    from a concurrent write shows in the Undo toast ("Rui changed this too:
    kept yours · Undo").
  - The change feed's `table_changed` refreshes open views.
- **Undo toasts** for every write.
- **Keys:** `n r` for a new row, and ⌘↵ to open the row. Shortcuts are
  registered in the catalogue and shown in the `?` sheet.
- **i18n:** en, pt-PT, es-ES and fr, including type names, operators and
  summaries.
- **Fit:** `pnpm test:fit` at 375 and 1280 in every locale.
- **Recording:** Storybook idle stories with a stateful mock, recorded at
  375 px and desktop.

## Numbers

Tracking patch `0019` (plan v19) if tracked events change, and migrations
from `20261111010000` if any are needed. Both were reserved by the merge
coordinator.

## The flag

This is the same `CAMPBOOKS_PAGES__TABLES__ENABLED`. With it off, none of
these screens, slash items or tree entries appear.
