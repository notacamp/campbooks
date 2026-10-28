## ADDED Requirements

### Requirement: Save a thread to a page
With pages enabled, the reader SHALL offer "Save to page" in its More menu
and through a shortcut. It opens a dialog to pick an existing page (filtered
by name, pages already linking the thread first) or a new private page, and
what to add: a live thread block, a copy of the messages (saying who reads
the page, from its effective readers, and whether it is sent to Notion), or
Scout's summary (shown as Scout's card before anything is added; hidden when
Scout is unavailable). Attachments can be
included. Every save SHALL show an Undo toast.

#### Scenario: A live block in an existing page
- **WHEN** Ana saves "Kickoff agenda" to "Client onboarding" as a live thread block
- **THEN** the page ends with the thread block and a toast offers "Open page" and "Undo"

#### Scenario: Scout not configured
- **WHEN** the `complete` role is not configured and Ana opens "Save to page"
- **THEN** the dialog offers the live block and the copy, but not Scout's summary

#### Scenario: A copy to a shared page says who will read it
- **WHEN** Ana chooses a copy of the messages for "Acme", which Rui reads through a grant on the page above it
- **THEN** the dialog says Rui will read it before Ana saves

#### Scenario: Off while the server does not offer it
- **WHEN** mail and Scout in pages is off on the server
- **THEN** the reader has no "Save to page" entry and its shortcut does nothing

#### Scenario: An attachment over the cap
- **WHEN** Ana includes attachments and one is over 20 MiB
- **THEN** the dialog says which file is too large and nothing is added
