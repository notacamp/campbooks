## MODIFIED Requirements

### Requirement: Notion in Settings and the import screen
With Notion enabled, Settings → Connections (not Mailboxes) SHALL offer Notion
with what it shares and who it belongs to. A connected card SHALL say "Notion ·
<name>", its health, and, only when it is healthy and one of its streams has a
`last_synced_at`, "checked Notion <time ago>" (Notion is pulled every few
minutes: the card SHALL NOT say "live" or "real time"). It SHALL list what it
brings in, how it stays in step and what Notion shares, and offer one row of
actions: Import pages, Open in Pages, Reconnect when it needs the member, and
Disconnect last. Disconnect SHALL ask in a dialog what stops (changes in Notion
no longer come in, edits no longer go back, Campbooks forgets its access) and
what stays (the imported pages as they are, everything in Notion), and SHALL
delete no page. "Import from Notion" SHALL list the pages the member's
connection can see, with sub-page counts and the note about blocks kept in
Notion, and SHALL import the ones picked. Returning from Notion SHALL land on
Connections.

#### Scenario: Picking pages
- **WHEN** Marta ticks "Product roadmap" and "Q4 OKRs" and imports
- **THEN** both appear in her Private place, marked as synced with Notion

#### Scenario: Checked a few minutes ago
- **WHEN** Marta's healthy Notion connection last ran 5 minutes ago
- **THEN** the card says "Connected · checked Notion 5 min. ago" and never "live"

#### Scenario: Disconnecting
- **WHEN** Marta presses Disconnect on the Notion card
- **THEN** a dialog lists what stops and what stays, and only its Disconnect button disconnects, keeping the pages
