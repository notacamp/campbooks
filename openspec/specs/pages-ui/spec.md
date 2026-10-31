# pages-ui Specification

## Purpose
The Pages place: the page tree, the block editor with the slash menu, mentions and Markdown shortcuts, the Share popover, Linked from, the trash, and a layout that fits every screen and language.

## Requirements

### Requirement: The Pages place
With pages enabled, the workspace frame SHALL offer a Pages place in the nav
(`g p`), with a tree of Workspace pages, Private pages, "Shared with you" and
the trash. `n p` SHALL create a page anywhere in the app and open it.
`⌘K` search SHALL find pages by title and text. On a phone the tree SHALL be
its own screen and a page SHALL open over it.

#### Scenario: A new page from anywhere
- **WHEN** a member presses `n p` in the inbox
- **THEN** a new private page opens with its title focused and an Undo toast

### Requirement: The editor
A page SHALL be edited in place, with a slash menu (`/`) for every block in
the schema, `@` mentions searched by name, Markdown shortcuts, a block handle
to drag, turn into, duplicate or delete, sanitised paste, and file and image
upload. The top bar SHALL say whether the page is saved, saving, or not saved
and retrying. Text editing SHALL undo and redo with `⌘Z` / `⇧⌘Z`.

#### Scenario: A to-do from Markdown
- **WHEN** a member types `[] ` at the start of an empty line
- **THEN** the line becomes a to-do

#### Scenario: Lost connection
- **WHEN** a sync fails because the network dropped
- **THEN** the top bar says the page is not saved and retrying, and the edit is sent once the network is back

### Requirement: Mentions on the page
A mention SHALL render with the record's current name and a link to it, and
SHALL render as "Something you can't open" when the viewer may not read the
record. A person SHALL be picked by name, never typed as an address.

#### Scenario: A mention of a person
- **WHEN** a member types `@Jon` and picks Jonas Weber
- **THEN** a person chip "Jonas Weber" is inserted and opens that person

### Requirement: Sharing a page
The Share popover SHALL show the page's place, the members it is shared with
and their role, let a member who may share add members by name with a role or
remove them, and say that mentions keep their own access. Every share change
SHALL show an Undo toast.

#### Scenario: Share with a teammate
- **WHEN** Ana shares "Weekly plan" with Rui as "Can edit"
- **THEN** Rui is listed with "Can edit" and an Undo toast is shown

### Requirement: Linked from
Each page SHALL show the pages that link to it below its body, as "Linked
from N places", with only the pages the viewer may read.

#### Scenario: A page nobody links to
- **WHEN** a page has no backlinks
- **THEN** no "Linked from" section is shown

### Requirement: Pages fit every screen and language
The Pages place SHALL have no horizontal page scroll at 375 px and 1280 px in
en, pt-PT, es-ES and fr. Wide blocks SHALL scroll inside their own box. Every
string SHALL be translated in the four locales.

#### Scenario: Phone width
- **WHEN** a page with a wide code block is opened at 375 px
- **THEN** the code block scrolls sideways inside its box and the page does not

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

### Requirement: The table page
A table page SHALL show its views as tabs, a one-row toolbar (filter chips,
Filter, Sort, New), and a grid whose cells edit in place with an editor for
each type. It SHALL support arrow keys to move, Enter to edit, Esc to
cancel, ⌘↵ to open the row and `n r` for a new row, and SHALL show footer
summaries. A new column SHALL be added from a type menu grouped as Basic,
Links and Filled in for you.

#### Scenario: Adding a column
- **WHEN** Marta adds a Campbooks link column named "Threads"
- **THEN** the column appears, and each cell can link threads picked by name

### Requirement: Board and row peek
The board SHALL show one column per group with its count and summary, and
cards that open the row. Opening a row SHALL show a side panel with its
properties and its page body, and "Open as page".

#### Scenario: Opening a row
- **WHEN** Marta opens "Acme Studio" from the board
- **THEN** a side panel shows its Stage, Owner, Next date, Value, Threads and its notes

### Requirement: Inline tables and phone layout
`/table` SHALL make a new inline table in a page, and `/link to table` SHALL
embed a view of an existing table. A reader without access to the table SHALL
see "Something you can't open". Grids and boards SHALL scroll inside their
own box, so a page never scrolls sideways, at 375 px and 1280 px in every
locale.

#### Scenario: A table on a phone
- **WHEN** "Clients" is opened at 375 px
- **THEN** the grid scrolls sideways inside its box, and the page does not

### Requirement: The address holds the view and the row
A table page's current view and the row opened beside it SHALL be in its
address, so a link opens them and Escape or Close takes the row out again.

#### Scenario: A link to a row
- **WHEN** Rui opens the link to the Acme row of the Clients board
- **THEN** the board opens with the Acme row beside it

### Requirement: Every change can be undone
Every change to a cell, a title, a row, a column, an option or a view SHALL
show an Undo, and a failure SHALL roll the cell back and say so until Retry.
A cell somebody else changed meanwhile SHALL be said in the toast, with the
member's value kept.

#### Scenario: A concurrent edit
- **WHEN** Marta saves a Value that Rui changed a moment before
- **THEN** the toast says someone else changed it too and that hers is kept, with Undo

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

### Requirement: Blocks are spaced to read
A page's blocks SHALL follow one spacing rhythm, at the top of the page and
inside callouts, quotes, toggles and list items: a small gap between blocks,
a larger one around boxed blocks (callouts, quotes, code, cards, images,
tables), more space above a heading than below it, and no extra space at the
top or bottom edge of the page or of a box. Bullet and numbered lists SHALL
show a marker for each item. A closed toggle SHALL show only its first line.
A to-do's checkbox SHALL sit on the first line of its text, and on a touch
screen each to-do SHALL be at least a finger tall.

#### Scenario: A heading and its text
- **WHEN** a page has a paragraph, then a heading, then a paragraph
- **THEN** the heading is further from the paragraph above it than from the paragraph under it, and neither paragraph touches it

#### Scenario: Two callouts in a row
- **WHEN** a page has two callouts one after the other
- **THEN** there is visible space between them, and the text inside each starts at its padding

#### Scenario: A closed toggle
- **WHEN** a toggle with two lines is closed
- **THEN** only its first line shows

#### Scenario: Two paragraphs in a row
- **WHEN** a page has two paragraphs one after the other
- **THEN** there is a gap between them, smaller than the gap above a heading

### Requirement: Ask Scout about a page
With mail and Scout in pages on, a page's More menu SHALL offer "Ask Scout",
which opens a Scout chat about that page. It SHALL be hidden when Scout is
unavailable, and the page toolbar SHALL stay one row.

#### Scenario: Asking about a page
- **WHEN** Ana chooses "Ask Scout" on "Client onboarding" and asks what is still open
- **THEN** Scout's chat is about that page and answers from it

#### Scenario: Scout not configured
- **WHEN** the `complete` role is not configured
- **THEN** the page's More menu has no "Ask Scout"

### Requirement: Callout tones are told apart
A callout SHALL show its tone by colour: neutral in grey, info in blue,
warning in red, each with a soft fill and a bar on its leading edge, in light
and dark themes. The info colour SHALL be its own design token, not the
workspace accent, so it reads the same whatever accent the workspace uses.

#### Scenario: Info and neutral callouts side by side
- **WHEN** a page has an info callout and a neutral callout
- **THEN** the info callout is blue and the neutral one grey, in light and in dark

#### Scenario: A workspace with a coloured accent
- **WHEN** the workspace accent is forest
- **THEN** the info callout is still blue
