- **Notion databases as tables (server side, off by default).** With
  `CAMPBOOKS_CONNECTORS__NOTION__DATABASES__ENABLED` on (and Notion, pages and tables on),
  `pages.import_from_notion` also takes Notion databases, each a private table of yours
  kept in step with Notion both ways: columns and values. A change in the same column or
  the same cell on both sides waits for your choice (`pages.get_table_sync`,
  `pages.resolve_table_conflict` with undo) and is sent to neither place until then.
  Removing a column that is Notion's too needs `confirm_remote_delete=true` (otherwise 400
  `confirmation_required`), removes it there as well, and `pages.restore_property` brings
  it back in both places with its values. Formula, rollup, people and the other kinds
  Notion alone has are read only, and Campbooks-only columns are never sent. A Notion
  database is never deleted and a row you delete is only moved to Notion's trash. The
  screens come in a later change.
