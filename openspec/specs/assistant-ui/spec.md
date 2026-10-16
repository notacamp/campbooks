# assistant-ui Specification

## Purpose
The question flow where a member tells Scout what they care about, offered
on Home after the first run and reachable again from Scout's memory.

## Requirements

### Requirement: The offer on Home
While the member's profile has no `asked_at`, Home SHALL offer "Tell Scout
what you care about" with Start (`T`) and Not now (`Shift+T`), each with
its shortcut hint. Start SHALL open the question flow; Not now SHALL record
`asked_at` and the offer SHALL not come back. When Not now can't be saved,
the offer SHALL stay and say so.

#### Scenario: Not now
- **WHEN** a new member chooses Not now
- **THEN** the profile records `asked_at`, Home stops offering, and Scout's memory still offers the questions

### Requirement: The question flow
The client SHALL ask, one per view: whose mail should always reach you,
what never needs you, and whether Scout should ask first or act and tell
you. Each view SHALL show its position ("1 of 3"), Next (`Enter`, also
from a ticked box), Skip (`S`) and Not now (`Esc`), and Back from
the second view on, each with its shortcut hint from the registry, and the
line "You can change this anytime in Scout's memory." No view SHALL
require an answer. The sender view SHALL offer the senders from
`conversations.list_senders` and a field that accepts an address or a
domain. Free-mail domains SHALL be refused with a sentence saying why.
The third question SHALL always be shown, with no client flag: the
proactivity it sets is read by taught rules (#482, on `next`) through the
`MemberPreferences` port. Not now SHALL keep the answers the member had
(waiting for them to load; when they can't be read it SHALL write nothing,
stay and say so), and SHALL close the flow only once saved, saying so when
it fails. It SHALL do nothing while a save is in flight.

#### Scenario: Skipping everything
- **WHEN** a member presses `S` on every view
- **THEN** the review view says nothing will change, and saving stores only `asked_at`

#### Scenario: Not now in the middle
- **WHEN** a member presses `Esc` on the second view
- **THEN** the flow closes, only `asked_at` is saved, and Home opens

#### Scenario: Not now fails
- **WHEN** saving Not now fails
- **THEN** the flow stays open and says Scout couldn't put the questions off

### Requirement: Every answer shown as its rule
On the sender and "never needs you" views, each selection SHALL show the
rule it becomes as the sentence Scout's memory uses. Before saving, a
review view SHALL list every rule grouped by inbox: the member's own
inboxes on; an inbox shared with others marked "For everyone" and off
until the member switches it on; inboxes the member only reads saying they
get no rule, and while the inboxes load or fail to load the view SHALL say
so rather than call them read-only. Save (`Enter`) SHALL create every rule
that is on (`state = active`, origin `onboarding`, one teaching per
answer), store the profile, and offer one Undo that removes them all and
puts the profile back; an Undo that fails SHALL say so.

While answers make rules but no inbox is on (the member's only teachable
inboxes are shared and none is switched on, or they only read their
inboxes), Save SHALL be disabled with a sentence saying why, and nothing
SHALL be saved. Save SHALL also wait, saying why, while the member's
current answers can't be read, so it never writes over one it never saw.
The done view and its notice SHALL count the rules the
server created, never the answers given; when none was created they SHALL
say what changed (the answer about acting, or nothing) instead of a count.

Rules SHALL be created one at a time. When one fails, those already created
SHALL stay and be remembered: Save (now "Try again") SHALL send only the
rest, with the same teaching, then the profile; when only the profile
failed, only the profile. The failure SHALL say how many were saved.

#### Scenario: Newsletters never need me
- **WHEN** a member picks "Newsletters and mailing lists"
- **THEN** the view shows "Newsletters and mailing lists never need you" before anything is saved

#### Scenario: Only a shared inbox, not switched on
- **WHEN** a member whose only teachable inbox is shared reaches the review with two rules and leaves "For everyone" off
- **THEN** Save is disabled with the reason, nothing is created, and the done view never says rules are on

#### Scenario: A retry after a failure
- **WHEN** the second of two rules fails to save and the member presses Try again
- **THEN** only the second rule is sent again, then the profile, and the done view says 2 rules are on

#### Scenario: Only the answer about acting changed
- **WHEN** a member skips the first two questions and chooses "Do it and tell me"
- **THEN** the done view says no rules were added and that Scout now acts and tells them, and the notice says the answers were saved

### Requirement: Accessible, localized, phone-ready
Every screen of this capability SHALL exist in en, pt-PT, es-ES and fr,
use only `lib/ui` components and tokens, show its shortcuts from the
registry (Next `Enter`, Skip `S`, Back `B`, Not now `Esc`), work at 375px with no horizontal scroll, and pass the axe checks
in light and dark.

#### Scenario: Portuguese at 375px
- **WHEN** a pt-PT member opens the flow on a 375px screen
- **THEN** every view reads in European Portuguese with diacritics and nothing scrolls sideways
