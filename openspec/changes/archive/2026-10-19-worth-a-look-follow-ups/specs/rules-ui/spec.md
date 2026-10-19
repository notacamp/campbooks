## ADDED Requirements

### Requirement: Highlight on the mail already here
When the goal is "Highlight them", the confirmation SHALL say that only the
latest unread one goes to Home (the preview reads "It matches N emails already
here. Only the latest unread one goes to Home."), and offer "Also apply it to
the latest one already here" instead of the count, and saving with it checked
SHALL raise one item for the newest unread match. For a shared inbox it SHALL
say "For everyone in <inbox>: it shows on everyone's Home there, and once anyone
opens it, it clears for all."

#### Scenario: Latest only
- **WHEN** Ana picks Highlight them for a sender with 41 unread emails already here
- **THEN** the confirmation offers "Also apply it to the latest one already here" and, once saved, Home shows one Worth a look item

#### Scenario: Shared inbox
- **WHEN** the suggestion is for a shared inbox and Ana picks Highlight them
- **THEN** the confirmation says it shows on everyone's Home there and clears for all once anyone opens it
