## ADDED Requirements

### Requirement: Importing from Notion
`pages.import_from_notion` SHALL take Notion pages that the caller's own
Notion connection can see, and SHALL make each a private page of the caller
under a parent they may write, with its sub-pages nested, each linked to its
Notion page. One Notion page SHALL be linked to at most one page per
connection.

#### Scenario: Importing a roadmap
- **WHEN** Marta imports "Product roadmap" with its 3 sub-pages
- **THEN** four private pages of Marta's are made and linked, and filled from Notion on the first run

#### Scenario: Someone else's connection
- **WHEN** Rui asks to import through Marta's Notion connection
- **THEN** the response is 404

### Requirement: Applying Notion changes block by block
Every block SHALL carry a stable block id. A Notion snapshot SHALL be
applied by comparing, for each mapped block, our version, Notion's version
and the base agreed at the last sync. Theirs changed alone: theirs is
applied. Ours changed alone: ours is kept for sending. Both changed: ours is
kept and a conflict block holding Notion's version is added after it. The
result SHALL be one Yjs update checked like any `pages.sync`. Notion kinds
without a mapping SHALL become `external` blocks that store only their kind
and Notion id, and that Campbooks never edits, sends or deletes.

#### Scenario: Different paragraphs on each side
- **WHEN** Marta edits paragraph 1 in Campbooks while Rui edits paragraph 3 in Notion
- **THEN** after the next syncs both pages hold both edits, and no conflict is raised

#### Scenario: The same paragraph on both sides
- **WHEN** Marta and Rui both change the launch date paragraph between two syncs
- **THEN** Marta's version stays, Notion's version appears beneath it as a conflict, and neither is sent until she chooses

### Requirement: Resolving a conflict
`pages.resolve_conflict` SHALL keep our block, use Notion's, or keep both as
ordinary blocks, SHALL move the base, SHALL let the block be sent again, and
SHALL be undoable.

#### Scenario: Use Notion's
- **WHEN** Marta chooses "Use Notion's" on the launch date conflict
- **THEN** her paragraph takes Notion's text, the conflict block goes away, and nothing is sent back for it

### Requirement: Mentions sent to Notion
A mention of a Campbooks record SHALL be sent to Notion as the text
"Campbooks link" linking to it, never with the record's subject, name or
text.

#### Scenario: A thread mention
- **WHEN** a synced page mentions the thread "Contract v2 — signed copy"
- **THEN** Notion receives "Campbooks link", and not the subject

### Requirement: Stopping a sync deletes nothing
Stopping a sync, disconnecting Notion, trashing the page on either side,
un-sharing it in Notion, or its connection's member leaving SHALL stop the
link with a reason. The page SHALL stay as it is, and nothing SHALL be
deleted in Notion.

#### Scenario: Disconnecting
- **WHEN** Marta disconnects Notion
- **THEN** her imported pages stay in Campbooks unchanged, marked as no longer syncing
