## ADDED Requirements

### Requirement: Notion in Settings and the import screen
With Notion enabled, Settings → Connections SHALL offer Notion with what it
shares and who it belongs to. "Import from Notion" SHALL list the pages the
member's connection can see, with sub-page counts and the note about blocks
kept in Notion, and SHALL import the ones picked.

#### Scenario: Picking pages
- **WHEN** Marta ticks "Product roadmap" and "Q4 OKRs" and imports
- **THEN** both appear in her Private place, marked as synced with Notion

### Requirement: Notion's installation address
The app SHALL serve `/connect/notion`, outside any workspace, for Notion's
listing to point at. A signed-out browser SHALL sign in and come back to it.
With one workspace it SHALL start the Notion connect flow for that workspace
at once; with several it SHALL first ask "Which workspace should Notion
connect to?" and then start it. The flow is the Settings card's own (members
only, bound to the browser that started it): this page only gets there. When
the instance does not offer Notion it SHALL say "Notion isn't available on
this instance", and when the person is not a member it SHALL say only members
can connect Notion.

#### Scenario: Installing from Notion's listing with two workspaces
- **WHEN** Marta, signed in to Acme and Household, opens `/connect/notion`
- **THEN** she is asked which workspace Notion should connect to, and choosing Acme sends her to Notion's own consent screen

#### Scenario: An instance without Notion
- **WHEN** someone opens `/connect/notion` where Notion is off
- **THEN** the page says Notion isn't available on this instance, with a way back, and nothing is started

### Requirement: A synced page says so
A linked page SHALL show its sync state ("Synced with Notion · 1 min ago",
paused, or stopped with its reason), "Open in Notion" and "Stop syncing". An
`external` block SHALL show as "Kept in Notion" with its kind and "Edit in
Notion". A conflict SHALL show Notion's version under ours, with Keep mine,
Use Notion's and Keep both, and an Undo after choosing. Sharing a synced page
to Workspace SHALL say that edits go to Notion through its owner's
connection.

#### Scenario: A conflict on the page
- **WHEN** a page holds a conflict for the launch date paragraph
- **THEN** the page header says one change is waiting, and the conflict shows both versions with the three choices
