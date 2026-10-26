## Why

Pages hold writing, but a lot of what a small business keeps is a list of
things with the same fields: clients, projects, suppliers, content plans.
In Notion those live in databases. The owner made tables the next Pages
priority (2026-10-25), ahead of mail and Scout in Pages. They are also the
ground for syncing Notion databases next (cloud #377), so the model must
hold a Notion database without a remodel.

Owner decisions (2026-10-25/26):

- **Views:** Table and Board in v1. Views are stored generically, so List,
  Calendar and Gallery can be added later with no data change.
- **Computed columns:** no formula or rollup editor in v1. When Notion
  databases sync (step 3), their formula and rollup values come across
  read-only.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/365 (epic
#363). Prototype: board "Tables" on
https://claude.ai/artifact/B4DKyoW5itjeAhcgAaY2rs

Modules touched:

- **`pages`**: a table is a page whose rows are pages. This adds typed
  properties, row values, views and the table query.
- **`search`**: row titles and text values are searchable through the pages
  `SearchSource`.
- **Client**:
  - the table page, with table and board views;
  - cell editors;
  - the row peek;
  - the new-column menu;
  - filters and sorts;
  - the inline table block.

**ADR 0022** (added by this change): tables live in `pages`, and their
property types are written as a self-contained library that ADR 0008's
future `records` module can adopt. It amends ADR 0008's plan that
user-defined record types wait for `records`.

## What Changes

- **A table** is a page of kind `table`. Its rows are pages of kind `row`
  under it, so access, trash, search, mentions and backlinks all work as
  they do for pages, and every row can hold a full page body.
- **Typed properties.** Every type has a Notion counterpart:
  - title, text, number (with format), select, multi-select, status (with
    to-do, doing and done groups), date (or range, optionally with time),
    checkbox, person (a teammate), files, URL, email and phone;
  - relation (rows of another table, one-way or paired two-way);
  - Campbooks link (threads, tasks, events, people, as references only);
  - created and edited (time and who), and ID (a prefix plus a number).
  Select options are kept by id, so renaming one changes every row. The
  types `notion_formula`, `notion_rollup`, `notion_people` and `external`
  are reserved for read-only values from Notion (step 3), and nobody can
  create them in v1.
- **Views:** table and board, each with:
  - filters (AND/OR groups);
  - sorts;
  - visible properties, their order and widths;
  - per-column summaries in the table (count, sum, average, min, max,
    percent checked);
  - for the board, a group-by on a select or status, plus card properties.
  Dragging a card between groups changes its value. Each table has at least
  one view.
- **Inline tables:** a `tableView` block shows a view of a table inside any
  page. A reader who can't read the table sees "Something you can't open".
- **Every edit has an undo:** cells, properties, options, views and rows.
  Concurrent edits to different cells never collide. The same cell changed
  twice keeps the later one, and the earlier writer gets the undo toast.
- **Off by default** behind `CAMPBOOKS_PAGES__TABLES__ENABLED`.

## Non-goals

- Formula and rollup editors. Notion values arrive read-only in step 3.
- List, Calendar, Gallery, Timeline and Chart views. The view model reserves
  their types.
- CSV import and export, row templates, recurring rows, automations on rows,
  and per-row permissions.
- Syncing Notion databases. That's cloud #377, built on this model.
- Scout filling or querying tables. That's in #364.
