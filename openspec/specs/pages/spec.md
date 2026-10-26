# pages Specification

## Purpose
Pages a member writes next to their mail: a tree of private and workspace pages whose body is one Yjs document merged on the server, shared with members as reader, writer or manager down the tree, mentioning records by reference only, with backlinks, a 30-day trash and events that carry ids and enums only.

## Requirements

### Requirement: Pages scopes and flag
The `pages` module SHALL declare the scopes `pages:read` and `pages:write`,
and every endpoint it adds SHALL require one of them. While
`CAMPBOOKS_PAGES__ENABLED` is off, every pages endpoint SHALL answer 404, no
pages job SHALL do work, and no pages setting SHALL stop the server from
starting.

#### Scenario: Token without the write scope
- **WHEN** a client with only `pages:read` creates a page
- **THEN** the response is 403 as `application/problem+json`

#### Scenario: Flag off
- **WHEN** `CAMPBOOKS_PAGES__ENABLED` is unset and a member lists pages
- **THEN** the response is 404 and the server started normally

### Requirement: A page
A page SHALL hold a title (0–200 characters after trimming, with bidi,
zero-width and control characters removed), an optional icon from a fixed
set, an optional parent page, a position among its siblings, a place
(`private` or `workspace`, inherited from its top-level ancestor), the member
who created it, who last edited it and when, and a body. A page SHALL be at
most 12 levels deep, and a page SHALL never be moved under itself or one of
its sub-pages. Every table of the `pages` schema SHALL be isolated per
workspace by forced row-level security.

#### Scenario: A new private page
- **WHEN** Ana creates a page with no parent in `private`
- **THEN** the page is top-level, private, created and owned by Ana, with an empty body

#### Scenario: A move into its own sub-page
- **WHEN** Ana moves "Team handbook" under its sub-page "Holidays"
- **THEN** the response is 409 and nothing moves

### Requirement: The body is a merged document
A page's body SHALL be a Yjs document. `pages.sync` SHALL accept the caller's
state vector and an optional update, apply the update, and answer the update
the caller is missing with the new state vector, so concurrent edits from
several writers all survive. The server SHALL refuse with 400, and store
nothing, an update that adds an element or attribute outside the block schema
in design.md, a link whose scheme is not `https`, `http` or `mailto`, a file
or image naming a blob the caller may not use, an update over 256 KiB, or one
that would make the document larger than 4 MiB or 20,000 blocks.

#### Scenario: Two writers at once
- **WHEN** Ana and Rui each add a paragraph to the same workspace page from the same starting state
- **THEN** after both sync, both paragraphs are in the page for both of them

#### Scenario: A script link
- **WHEN** a client syncs an update with a link to `javascript:alert(1)`
- **THEN** the response is 400 and the page is unchanged

### Requirement: Who may open a page
A private page SHALL be readable and changeable by its creator only, unless
it is shared. A workspace page SHALL be readable and writable by every
member; only its creator, a workspace owner, or a `manager` SHALL share,
delete, or move it out of `workspace`. A grant of `reader`, `writer` or
`manager` on a page SHALL apply to the page and to every sub-page without a
grant of its own for that member, and a caller SHALL hold the highest role
any of these gives. A caller who may not read a page SHALL get 404 for every
action on it, and the page SHALL be left out of lists, search and backlinks.

#### Scenario: A teammate's private page
- **WHEN** Rui opens Ana's private page that was never shared with him
- **THEN** the response is 404

#### Scenario: A grant reaches sub-pages
- **WHEN** Ana shares her private page "Weekly plan" with Rui as `reader`
- **THEN** Rui can read "Weekly plan" and its sub-pages, and a write by Rui answers 403

### Requirement: Mentions keep their own access
A mention or embed SHALL store only its type and id (for `person`, the
cleaned address). The server SHALL never copy a mentioned record's subject,
name or text into the page, its links, events, search or logs. A mentioned
record SHALL be shown to a reader only through that record's own read
operation with the reader's access.

#### Scenario: A thread the reader cannot open
- **WHEN** Ana mentions a thread from her own inbox on a workspace page and Rui, who cannot read that inbox, opens the page
- **THEN** Rui sees that something is linked there, and not its subject or sender

### Requirement: Backlinks
`pages.backlinks` SHALL answer, for a record the caller may read, the pages
the caller may read that mention or embed it, the most recently edited first.
For a record the caller may not read it SHALL answer 404.

#### Scenario: A task linked from a page
- **WHEN** Ana asks for the backlinks of her task "Send the welcome pack", mentioned on "Client onboarding"
- **THEN** the answer lists "Client onboarding"

### Requirement: Trash
Trashing a page SHALL trash it and its sub-pages together, restorable as one
for 30 days, after which a daily job SHALL delete them for good. Restoring
SHALL put the page back under its parent, or at the top level of its place if
the parent is gone. `pages.purge` SHALL delete a trashed page for good
immediately, only for a caller who may delete it.

#### Scenario: Restore after the parent was purged
- **WHEN** "Holidays" was trashed alone, then its parent "Team handbook" was purged, and Ana restores "Holidays"
- **THEN** "Holidays" is back as a top-level page in `workspace`

### Requirement: Page events carry no content
Every pages write SHALL publish `pages.page_changed.v1` in its transaction
with ids, enums, a growing revision and the kinds of change only: no title,
no text, no address. Body changes from `pages.sync` SHALL be published at
most once per page every 5 seconds.

#### Scenario: A title change
- **WHEN** Ana renames a page
- **THEN** one `pages.page_changed.v1` with kind `meta` is published, and it holds no title

### Requirement: Workspace purge
When a workspace is deleted, the pages module SHALL remove all its pages,
updates, snapshots, grants and links, and release its blob references.

#### Scenario: A deleted workspace
- **WHEN** the workspace purge runs for a workspace with pages
- **THEN** no row of the `pages` schema remains for it

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
