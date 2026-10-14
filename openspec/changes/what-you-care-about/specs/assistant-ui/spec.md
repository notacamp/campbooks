## Purpose

The question flow where a member tells Scout what they care about, at first
run and again from Scout's memory.

## ADDED Requirements

### Requirement: The question flow
The client SHALL ask the five questions one per view, in order: what you
use Campbooks for (with an optional "In a few words, what do you do?"),
what matters most (up to three), whose mail should always reach you, what
never needs you, and whether Scout should ask first or act and tell you.
Each view SHALL show its position ("2 of 5"), Next (`Enter`), Skip (`S`)
and Later (`Esc`), and Back from the second view on, each with its
shortcut hint from the registry, and the line "You can change this anytime
in Scout's memory." No view SHALL require an answer. The sender views
SHALL offer suggestions from `conversations.list_senders` and a field that
accepts a name, an address or a domain.

#### Scenario: Skipping everything
- **WHEN** a member presses `S` on every view
- **THEN** the review view says nothing will change, and saving stores only `asked_at`

#### Scenario: Later
- **WHEN** a member presses `Esc` on the second view
- **THEN** the flow closes, nothing is saved but `asked_at`, and Home offers the flow again for 7 days

### Requirement: Every precise answer shown as its rule
On the sender and "never needs you" views, each selection SHALL show the
rule it becomes as the sentence Scout's memory uses, with the count of
matching emails and up to three samples from `conversations.preview_rule`.
Before saving, a review view SHALL list every rule and every sentence Scout
will keep in mind, offer "Also apply to the N emails you already have"
(on by default), and Save (`Enter`). Rules are grouped by inbox: the
member's own inboxes are on; an inbox shared with others is marked "For
everyone" and off until the member switches it on; inboxes the member only
reads say they get no rule. Saving SHALL create every rule that is on
(`state = active`, origin `onboarding`) and the profile at once and show a
toast with Undo that removes all of them.

#### Scenario: Newsletters never need me
- **WHEN** a member picks "Newsletters and mailing lists"
- **THEN** the view shows "Newsletters and mailing lists never need you · 312 emails" with three samples before anything is saved

### Requirement: Accessible, localized, phone-ready
Every screen of this capability and its additions to Scout's memory SHALL exist in en, pt-PT, es-ES and fr,
use only `lib/ui` components and tokens, show its shortcuts from the
registry, work at 375px with no horizontal scroll, and pass the axe checks
in light and dark.

#### Scenario: Portuguese at 375px
- **WHEN** a pt-PT member opens the flow on a 375px screen
- **THEN** every view reads in European Portuguese with diacritics and nothing scrolls sideways
