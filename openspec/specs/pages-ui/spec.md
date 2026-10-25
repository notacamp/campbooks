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
