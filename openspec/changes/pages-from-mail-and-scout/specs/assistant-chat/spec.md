## ADDED Requirements

### Requirement: Scout reads pages
With pages enabled, Scout SHALL be able to find pages the member may read
(`find_pages`: ids, titles, places and edit dates only) and read one as text
(`read_page`, within the tool result cap). A chat MAY be about a page. Page
text, titles and Markdown SHALL never be logged or put into events.

#### Scenario: A question about the member's own notes
- **WHEN** Ana asks Scout "what did we agree for Acme's first invoice?" and her page "Client onboarding" says it
- **THEN** Scout reads that page and answers from it, naming the page

#### Scenario: A page the member cannot read
- **WHEN** Rui asks Scout about the contents of Ana's private page
- **THEN** `find_pages` does not list it and `read_page` answers that it is not found

### Requirement: Scout proposes page changes
Scout SHALL change pages only through `propose_page_change`: a section at
the end of a page (`pages.append`) or a new private page (`pages.create`),
with its sources. A proposal SHALL be checked when stored (the page is
writable, the Markdown is within the caps, and every source is readable by
the member), SHALL run only when the member confirms it, and SHALL be
undoable (`pages.unappend`, `pages.trash`). Scout SHALL never edit or remove
existing blocks.

#### Scenario: Kickoff notes from mail
- **WHEN** Ana asks Scout to write kickoff notes for Acme on "Client onboarding"
- **THEN** a card shows the section with its source threads, and the page is unchanged until Ana chooses "Add to page"
