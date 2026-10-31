## ADDED Requirements

### Requirement: The list says whether mail and Scout are on
`pages.list` SHALL say whether mail and Scout in pages is on (`mail_scout`),
whether or not the caller has any page yet, so a client offers "Save to page"
and "Ask Scout" about a page only when they work.

#### Scenario: On, with no pages yet
- **WHEN** Marta, who has no pages, lists her pages with mail and Scout on
- **THEN** the answer has no pages and says `mail_scout` is true

#### Scenario: Off
- **WHEN** Marta lists her pages with mail and Scout off
- **THEN** the answer says `mail_scout` is false
