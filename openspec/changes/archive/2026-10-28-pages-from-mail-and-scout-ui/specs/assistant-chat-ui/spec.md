## ADDED Requirements

### Requirement: Page change cards
A proposal for `pages.append` or `pages.create` SHALL show as a page change
card. The card shows the target page (or the new page and the page it goes
under), who will read it (only you, named teammates with a count of the
others, or everyone), whether it is sent to Notion too, the whole Markdown
rendered as the page would show it (folded past a length with "Show all"),
and the source threads. It offers "Add to page", "Edit first" and "Discard".
It SHALL never call a page private when anyone else can read it, and SHALL
offer "Add to page" only when it can show the whole Markdown, who will read
the page and, when it applies, the Notion line. Nothing SHALL change until
the member chooses "Add to page", and an added card SHALL offer "Open page"
and "Undo".

#### Scenario: A section for a page Rui reads
- **WHEN** Scout proposes a section for "Acme", which Rui reads through an inherited grant
- **THEN** the card says "You, Rui" will read it, shows the section and its sources, and the page is unchanged

#### Scenario: Edit first
- **WHEN** Ana chooses "Edit first", shortens the section and chooses "Add to page"
- **THEN** the shortened section is added and the card offers "Open page" and "Undo"

#### Scenario: Undo
- **WHEN** Ana chooses "Undo" on an added section
- **THEN** the section's blocks are removed from the page, except blocks someone edited since

#### Scenario: A proposal that does not say who reads the page
- **WHEN** a page proposal arrives without its readers or its Markdown
- **THEN** the card shows no "Add to page"
