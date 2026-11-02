## ADDED Requirements

### Requirement: Connections section
Settings SHALL have a Connections section (`/settings/connections`) between
Mailboxes and Calendar, in the sidebar and on the phone's chip row, for apps
that are not mail. Its lead SHALL say each connection is the member's own, and
it SHALL end with a quiet line pointing to Mailboxes and Calendar. Mailboxes
SHALL list only mailboxes and point to Connections for Notion and other apps.

#### Scenario: Looking for Notion
- **WHEN** Marta opens Mailboxes
- **THEN** she sees no Notion card, and a line "Notion and other apps are in Connections" that opens Connections
