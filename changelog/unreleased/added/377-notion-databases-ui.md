- **Notion databases as tables, the screens (off by default).** With
  `CAMPBOOKS_CONNECTORS__NOTION__DATABASES__ENABLED` on, Import from Notion lists the
  databases you shared beside your pages (`connectors.list_notion_data_sources`, yours
  alone), and ticking one imports it as a table. A synced table says "Synced with Notion",
  with Open in Notion, Stop syncing and the changes to look at. Formulas, rollups and
  people show a lock that says why and cannot be edited; Campbooks-only columns say so;
  what Notion can't do (a status column made here, renaming, recolouring or removing an
  option) says it stays in Campbooks. A cell changed in both places shows Notion's value
  under yours with Keep mine and Use Notion's, a column changed in both shows a banner,
  and every choice has Undo. Removing a synced column asks "Remove from both places?",
  names the 30 days Campbooks keeps its values, and Undo puts it back in both.
