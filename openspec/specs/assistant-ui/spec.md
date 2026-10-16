# assistant-ui Specification

## Purpose
The question flow where a member tells Scout what they care about, offered
on Home after the first run and reachable again from Scout's memory.

## Requirements

### Requirement: The offer on Home
While the member's profile has no `asked_at`, Home SHALL offer "Tell Scout
what you care about" with Start and Not now. Start SHALL open the question
flow; Not now SHALL record `asked_at` and the offer SHALL not come back.

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

#### Scenario: Skipping everything
- **WHEN** a member presses `S` on every view
- **THEN** the review view says nothing will change, and saving stores only `asked_at`

#### Scenario: Not now in the middle
- **WHEN** a member presses `Esc` on the second view
- **THEN** the flow closes, only `asked_at` is saved, and Home opens

### Requirement: Every answer shown as its rule
On the sender and "never needs you" views, each selection SHALL show the
rule it becomes as the sentence Scout's memory uses. Before saving, a
review view SHALL list every rule grouped by inbox: the member's own
inboxes on; an inbox shared with others marked "For everyone" and off
until the member switches it on; inboxes the member only reads saying they
get no rule. Save (`Enter`) SHALL create every rule that is on (`state = active`,
origin `onboarding`, one teaching per answer), store the profile, and
offer one Undo that removes them all and puts the profile back.

#### Scenario: Newsletters never need me
- **WHEN** a member picks "Newsletters and mailing lists"
- **THEN** the view shows "Newsletters and mailing lists never need you" before anything is saved

### Requirement: Accessible, localized, phone-ready
Every screen of this capability SHALL exist in en, pt-PT, es-ES and fr,
use only `lib/ui` components and tokens, show its shortcuts from the
registry, work at 375px with no horizontal scroll, and pass the axe checks
in light and dark.

#### Scenario: Portuguese at 375px
- **WHEN** a pt-PT member opens the flow on a 375px screen
- **THEN** every view reads in European Portuguese with diacritics and nothing scrolls sideways
