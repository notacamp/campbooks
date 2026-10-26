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


## Deviations while building

- **`kind` also says `row`.** A row is a page: `pages.get` of one answers `row`, which the
  row page and the peek need. `pages.list` still never lists rows.
- **`rows` on `pages.query_table`** (up to 100 row ids) and **`tables` on `pages.list`** were
  not in the plan. The first reads the rows a relation cell names, and a row opened as a page,
  without paging a table; the second is how a client knows tables are on (there is no other
  capability call).
- **The address holds the view and the row** (`?view=`, `?row=`), so a link opens a board with
  a row beside it. An inline table keeps its own.
- **The components are the app's, not the package's.** They name tables and rows, so they sit in
  `lib/ui/components/Table` beside `PageTree`, built from the package's. `@notacamp/ui` gained
  only what is generic: the table and option colour tokens, five icons and the `table` shortcut
  scope.
- **No tracked events.** Every screen action is one of part 1's operations, already tracked.
- **No `table_changed` subscription.** Open views refresh through the change feed's existing
  `pages.page` mapping (`get_table` and `query_table` read that record).
- **Concurrent edits do not say who.** The write's answer carries the replaced value, not the
  person, so the toast says "Someone else changed Value too: yours is kept".
- **A text cell is edited as plain text.** Links and marks it came with (from Notion) show as they
  were and are flattened only if the cell is edited.
- **A percent number shows its value with %** (42 reads 42 %), as it is stored.
- **Cards keep no order inside a column**: the API has no row position to move, so a board orders
  by the view's sorts. Dropping a card on a column writes its stage.
- **Option colour is a select**, not a row of swatches, so it is a real, labelled control.
- **A row is deleted from its peek** ("Move to trash", with Undo).
- **Rows are windowed** past 120 loaded rows (the first row's height is measured), and more load
  as the end scrolls into view, or with the "Load more rows" button.
