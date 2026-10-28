## ADDED Requirements

### Requirement: Importing and showing synced Notion databases
With Notion databases enabled, Import from Notion SHALL list shared databases
beside pages and import the ticked ones as tables. A synced table SHALL say
it is synced with Notion, with Open in Notion, Stop syncing and the number of
changes to look at. Read-only Notion columns SHALL show a lock with the
reason and SHALL not be editable. Changes Notion cannot make (a status column
made here, renaming, recolouring or removing an option) SHALL say they stay in
Campbooks.

#### Scenario: A formula column
- **WHEN** Marta opens the synced Clients table with a Notion formula column "Score"
- **THEN** Score shows a lock saying it is computed in Notion, and its cells cannot be edited

### Requirement: Conflicts and removal on a synced table
A cell changed on both sides SHALL show Notion's value under ours with Keep
mine and Use Notion's. A column changed on both sides SHALL show a banner
above the table with both versions. Every choice SHALL offer Undo. Removing a
synced column SHALL ask "Remove from both places?", name how many values go
and the 30-day restore, and send the confirmation only when the member
confirms.

#### Scenario: Removing Score
- **WHEN** Marta removes the Score column and confirms "Remove from both places"
- **THEN** the column is removed in Campbooks and in Notion, and an Undo toast offers to bring it back in both
