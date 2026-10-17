## MODIFIED Requirements

### Requirement: The question flow
The client SHALL ask, one per view: whose mail should always reach you,
what never needs you, and whether Scout should ask first or act and tell
you. Each view SHALL show its position ("1 of 3"), Next (`Enter`, also
from a ticked box), Skip (`S`) and Not now (`Esc`), and Back from
the second view on, each with its shortcut hint from the registry, and the
line "You can change this anytime in Scout's memory." No view SHALL
require an answer. The sender view SHALL offer the senders from
`conversations.list_senders`. Both sender views SHALL have a filter box
that finds senders by name, address or domain, so the member never needs
to know an address by heart. Typing SHALL filter the cards at once, and
from two characters SHALL ask `list_senders` with `q`. Senders already
ticked SHALL stay listed while filtering. A full address or a domain that
matches no sender SHALL show as its own card to tick. With text in the
box, `Enter` SHALL tick the first match and clear the box, and `Esc` SHALL
clear it. When nothing matches, the view SHALL say so. The "never needs
you" view SHALL not suggest senders before the member types. Free-mail
domains SHALL be refused with a sentence saying why.
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

#### Scenario: Finding someone by name
- **WHEN** a member types "joana" in the filter box and Joana Reis is not among the suggested senders
- **THEN** the cards show Joana Reis with her address, found through `list_senders?q=joana`, and ticking her shows the rule she becomes

#### Scenario: Someone not in the mail yet
- **WHEN** a member types "new@studio.example" and no sender matches
- **THEN** a card "new@studio.example" says there is no mail from them yet, and ticking it adds them
