## MODIFIED Requirements

### Requirement: Acting from Home
Enter SHALL open an item in place when no action is highlighted, showing the
email the need is about as the reader renders it (sanitised, in the sandboxed
frame, remote images held back, its links leading nowhere when the need is a
warning), a few lines deep with "Show the whole email" to read it in place,
and "How Campbooks decided" (closed by default); the sender's quote SHALL be
shown only when there is no email to show. ↑/↓ and J/K
SHALL move between items (closing an open one), R SHALL open the
conversation's composer, E SHALL mark it done (`conversations.resolve_need`),
S SHALL open the Snooze dialog (`attention.snooze`), X SHALL mark it not
needed (a warning's "It's safe", a question's No), and 1 to 3 SHALL run the
item's own actions (a Review question's Yes and No on 1 and 2). Each action
that takes the item off the list SHALL do so at once, show a toast with Undo,
and Z SHALL undo it. The selection SHALL be an explicit item from the first
render; when a live update removes the selected item, its neighbour at the
same position SHALL take the selection, and E and X SHALL act only on the
selected item. When the server refuses or fails, the item SHALL come back and
the toast SHALL say so and stay until Retry or dismiss (a changed need SHALL
be said as such, with no Retry). The sender's words and name SHALL be
isolated from the page's text direction. Items in an inbox the user may only
read SHALL offer no Reply, Done or Not needed.

#### Scenario: Done and Undo
- **WHEN** the user presses J then E on Pedro's lease, then Z
- **THEN** the item leaves the list with the toast "Done · Pedro Alves", and Z brings it back

#### Scenario: The selected item goes away
- **WHEN** the selected item is cleared elsewhere while Home is open
- **THEN** the item below it is selected, and E acts on that one

#### Scenario: Later
- **WHEN** the user presses S and chooses Monday morning
- **THEN** the item leaves the list and comes back on Monday morning

#### Scenario: Reading the email before deciding
- **WHEN** the user selects Marta's item and presses Enter
- **THEN** her email shows under the item a few lines deep, and "Show the whole email" shows all of it in place

#### Scenario: A warning's links lead nowhere
- **WHEN** the user opens a phishing warning on Home
- **THEN** its email shows with links that open nothing, and a line says links are off in mail that may be phishing
