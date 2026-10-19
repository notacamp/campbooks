## MODIFIED Requirements

### Requirement: What needs you
When the user can see at least one inbox, Home SHALL show their open
attention items (`attention.list`) in groups, in the order Review, Reply,
Decide, Do, Pay, leaving out empty groups, each item in the list's order. The
hero SHALL say how many things need the user, with the Visor `alert` when
Review has an item. The selected item SHALL be shown by the selected background and a full focus
ring, never by a coloured stripe on one edge. Each item SHALL show who it is from, its sentence (from a
template chosen by reason and by the rule that decided it, in the user's
language), the sender's sentence quoted on one line, and, at the row's end,
how long it has waited above its due date or overdue. The age SHALL count
from when the item's cause arrived (`since`), in the user's language: minutes
or hours the same day, "Yesterday" and days ago in the week after, the date
from a week on (with the year when it is another year's), with the full date
and time on hover; every row SHALL show it, dated or not. Rows SHALL come from
the owning module's batch read (`conversations.list` with `ids`), not from
`attention`.

#### Scenario: A mixed morning
- **WHEN** the user has a phishing warning, two replies, a dated ask and a bill
- **THEN** Home says "5 things need you", Review comes first with the warning, then Reply, Do and Pay, and nothing shows a percentage

#### Scenario: Due soonest first
- **WHEN** the Reply group holds a question due Wednesday and an older one with no date
- **THEN** the one due Wednesday is first

#### Scenario: How long it has waited
- **WHEN** Marta's question arrived at 10:00 today, Pedro's on Monday and the bill on 2 October, and it is Friday 15:00
- **THEN** Marta's row says "5 hours ago", Pedro's "4 days ago" and the bill's "2 Oct" above "Due 30 Oct", each with its full date on hover

### Requirement: Acting from Home
Enter SHALL open an item in place when no action is highlighted, showing
Scout's summary of the email first when there is one (under "Scout's summary",
marked with the visor, as text only; nothing when no summary is written, no
model may read mail here or the call failed), then the email the need is
about as the reader renders it (sanitised, in the sandboxed
frame, remote images held back, its links leading nowhere when the need is a
warning), a few lines deep with "Show the whole email" to read it in place,
and "How Campbooks decided" (closed by default); the sender's quote SHALL be
shown only when there is no email to show. ↑/↓ and J/K SHALL move between
items and open the item moved onto, as a click on it does (Enter or Esc
closes it again); when an open item leaves the list, the item taking its
place SHALL open, so reading goes on item by item. R SHALL open the reply dialog for the item's
conversation, E SHALL mark it done (`conversations.resolve_need`), S SHALL
open the Snooze dialog (`attention.snooze`), X SHALL mark it not needed (a
warning's "It's safe", a question's No), and 1 to 3 SHALL run the item's own
actions (a Review question's Yes and No on 1 and 2). Each action that takes
the item off the list SHALL do so at once, show a toast with Undo, and Z
SHALL undo it. The selection SHALL be an explicit item from the first
render; when a live update removes the selected item, its neighbour at the
same position SHALL take the selection, and E and X SHALL act only on the
selected item. When the server refuses or fails, the item SHALL come back
and the toast SHALL say so and stay until Retry or dismiss (a changed need
SHALL be said as such, with no Retry). The sender's words and name SHALL be
isolated from the page's text direction. Items in an inbox the user may only
read SHALL offer only Snooze: no Reply, Done, Not needed, Yes, No or It's
safe, since each of those writes in the inbox.

#### Scenario: Done and Undo
- **WHEN** the user presses J then E on Pedro's lease, then Z
- **THEN** the item leaves the list with the toast "Done · Pedro Alves", and Z brings it back

#### Scenario: Reading item by item with the keys
- **WHEN** the user presses J on Home, then E
- **THEN** J selects and opens Marta's item, showing her email as a click would, and after E the item below opens in her place

#### Scenario: The selected item goes away
- **WHEN** the selected item is cleared elsewhere while Home is open
- **THEN** the item below it is selected, and E acts on that one

#### Scenario: Later
- **WHEN** the user presses S and chooses Monday morning
- **THEN** the item leaves the list and comes back on Monday morning

#### Scenario: Reply from Home
- **WHEN** the user presses R on Marta's question
- **THEN** the reply dialog "Reply to Marta" opens over Home, and the item stays selected behind it

#### Scenario: Reading the email before deciding
- **WHEN** the user selects Marta's item and presses Enter
- **THEN** her email shows under the item a few lines deep, and "Show the whole email" shows all of it in place

#### Scenario: Scout's summary first
- **WHEN** Marta's need has a summary and the user opens her item
- **THEN** "Scout's summary" with the visor shows what the email is about, above the email, and nothing is asked of the model

#### Scenario: No summary
- **WHEN** an item's need has no summary
- **THEN** the opened item shows the email without any summary or placeholder

#### Scenario: A warning's links lead nowhere
- **WHEN** the user opens a phishing warning on Home
- **THEN** its email shows with links that open nothing, a line says links are off in mail that may be phishing, and there is no Scout's summary
