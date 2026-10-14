# home-ui Specification

## Purpose
A workspace's Home once mail is connected: what needs the member, grouped by
reason and due soonest first, with keyboard actions, undo, Done today and an
honest empty state.

## Requirements

### Requirement: What needs you
When the user can see at least one inbox, Home SHALL show their open
attention items (`attention.list`) in groups, in the order Review, Reply,
Decide, Do, Pay, leaving out empty groups, each item in the list's order. The
hero SHALL say how many things need the user, with the Visor `alert` when
Review has an item. The selected item SHALL be shown by the selected background and a full focus
ring, never by a coloured stripe on one edge. Each item SHALL show who it is from, its sentence (from a
template chosen by reason and by the rule that decided it, in the user's
language), the sender's sentence quoted on one line, and its chips (due date
or overdue, urgent, how long it has waited). Rows SHALL come from the owning
module's batch read (`conversations.list` with `ids`), not from `attention`.

#### Scenario: A mixed morning
- **WHEN** the user has a phishing warning, two replies, a dated ask and a bill
- **THEN** Home says "5 things need you", Review comes first with the warning, then Reply, Do and Pay, and nothing shows a percentage

#### Scenario: Due soonest first
- **WHEN** the Reply group holds a question due Wednesday and an older one with no date
- **THEN** the one due Wednesday is first

### Requirement: Acting from Home
Enter SHALL open an item in place with its full quote, its actions and "How
Campbooks decided" (closed by default). J and K SHALL move, R SHALL open the
conversation's composer, E SHALL mark it done (`conversations.resolve_need`),
L SHALL offer Later today, Tomorrow, Monday or a date (`attention.snooze`),
X SHALL mark it not needed, and 1 and 2 SHALL answer a Review question. Each
action SHALL take the item off the list at once, show a toast with Undo, and
Z SHALL undo it. The selection SHALL be an explicit item from the first
render; when a live update removes the selected item, its neighbour at the
same position SHALL take the selection, and E and X SHALL act only on the
selected item. When the server refuses or fails, the item SHALL come back with
a plain message (and a changed need SHALL be said as such). The sender's words
and name SHALL be isolated from the page's text direction. Items in an inbox
the user may only read SHALL offer no Reply, Done or Not needed.

#### Scenario: Done and Undo
- **WHEN** the user presses J then E on Pedro's lease, then Z
- **THEN** the item leaves the list with the toast "Done · Pedro Alves", and Z brings it back

#### Scenario: The selected item goes away
- **WHEN** the selected item is cleared elsewhere while Home is open
- **THEN** the item below it is selected, and E acts on that one

#### Scenario: Later
- **WHEN** the user presses L and chooses Monday
- **THEN** the item leaves the list and comes back on Monday morning

### Requirement: Done today
Home SHALL list what left the user's list today (`attention.list` with
`state=cleared`) as plain sentences naming the person and how it left ("You
replied to Marta Ferreira", "Ana marked Pedro Alves's lease done"), with Undo
for Done, Not needed and Later, and none for replies, archives and the trash.
On desktop it SHALL sit in the aside; below 48rem, in a fold after the list.

#### Scenario: Undo from the list
- **WHEN** the user chooses Undo next to "You marked Acme Hosting's bill done"
- **THEN** `conversations.reopen_need` is called and the item is back in Pay

### Requirement: Nothing needs you
When the user can see an inbox and has no open item, Home SHALL show the
Visor happy, "Nothing needs you", a line saying how many messages Campbooks
read today, and Done today.

#### Scenario: All clear
- **WHEN** the last item is marked done
- **THEN** Home shows "Nothing needs you" and the item in Done today

### Requirement: Home stays current
Home SHALL refetch its items when the change feed brings an `attention.item`
change and when the earliest snooze it knows of ends, and SHALL show the same
groups, actions and states at 375px (no horizontal scrolling, the tab bar's
Home badge with the count) and on desktop, in light and dark, in en, pt-PT,
es-ES and fr, passing axe.

#### Scenario: Mail arrives while Home is open
- **WHEN** a new question reaches the user's inbox while Home is open
- **THEN** it appears in Reply without a reload
