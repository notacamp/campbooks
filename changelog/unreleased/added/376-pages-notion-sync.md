- **Sync Pages with Notion** (off by default: `CAMPBOOKS_CONNECTORS__NOTION__ENABLED`). Connect your
  Notion account under Settings, pick the pages you shared with Campbooks in Notion's own screen, and
  "Import from Notion" brings them into your Private place; from then on an edit on either side
  reaches the other (about a minute from Notion with its webhook, or on a five-minute poll without
  one; about ten seconds after you stop typing from here), block by block, so different paragraphs
  edited on each side merge on their own. When the same paragraph changed on both sides, yours stays
  and Notion's version appears beneath it with "Keep mine", "Use Notion's" and "Keep both" (and
  Undo); nothing is thrown away until you choose. Blocks Pages lacks (columns, synced blocks,
  embeds, databases) show as "Kept in Notion" and are never edited or deleted from here. Mentions of
  your mail, tasks and events reach Notion as a plain "Campbooks link", never with their subject.
  Stopping, disconnecting or trashing a page ends the sync and deletes nothing on either side; a page
  is never trashed in Notion from here. Self-hosters: register a public Notion integration and set
  `CAMPBOOKS_NOTION__CLIENT_ID`, the secret and, for the webhook, `CAMPBOOKS_NOTION__WEBHOOK_SECRET`.
