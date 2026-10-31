## Why

In the table's grid a narrow column cuts a number or a sum with no sign that
there is more (`€9,600.(`), and on a phone the add-column "+" fell under the
first column, so "New row" appeared with a second "+" beside it.

Tracking: https://github.com/notacamp/campbooks-cloud/issues/387.

Modules touched:

- **Client only** (`clients/app`, `lib/ui` table parts): no server, migration,
  contract, tracked event or prompt change.

## What changes

- Cells (text, number, date, tags, links) end in an
  ellipsis when the column is too narrow, and carry the whole value in their title (so does a footer summary, which wraps).
- The header's add-column cell gets a grid track of its own (it fell to a second
  row under the first column), and the sticky first footer cell sits above what
  scrolls under it, so the phone's "New row" shows one "+".
