## ADDED Requirements

### Requirement: Pages are searchable
With pages enabled, search SHALL index each page's title and the plain text of
its blocks through the `pages` module's `SearchSource`, and SHALL answer a
page only to a caller who may read it. A trashed page SHALL leave the index,
and a restored one SHALL return to it.

#### Scenario: A page the caller may not read
- **WHEN** Rui searches for a word that appears only on Ana's private page
- **THEN** the page is not among Rui's results

#### Scenario: A trashed page
- **WHEN** Ana trashes a page and then searches for its title
- **THEN** the page is not among her results
