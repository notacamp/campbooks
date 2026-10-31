## ADDED Requirements

### Requirement: Mail and Scout follow the pages list
The client SHALL take whether mail and Scout in pages is on from `pages.list`
(the list the page tree already holds), with no request of its own. While the
list is loading, "Save to page" and "Ask Scout" are hidden. A workspace with
no pages yet SHALL be offered "Save to page" when it is on, whose "New page"
choice makes the first page.

#### Scenario: No pages yet
- **WHEN** Ana has no pages, mail and Scout are on, and she opens a thread
- **THEN** the reader offers "Save to page", and its dialog offers "New page"

#### Scenario: Off
- **WHEN** the list says `mail_scout` is false
- **THEN** the reader has no "Save to page" and a page's menu has no "Ask Scout"
