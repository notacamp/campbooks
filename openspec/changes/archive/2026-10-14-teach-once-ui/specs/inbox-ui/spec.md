## ADDED Requirements

### Requirement: Offering to do the same to lookalikes
After the server confirms that the user moved (M), archived (E), marked read
(U, on an unread conversation) or marked Not needed (X, not a warning) one
conversation in the inbox, the inbox SHALL ask `conversations.suggest_rules`
and, when there is a suggestion, show an offer docked above the toasts
(across the bottom below 48rem) that does not block the list: the Visor, the
question in plain words ("Move all 1,000 like this to Updates, and new ones
too?"), the likeness choices with their counts when there are several, up to
three sample subjects with their dates and how many more, whether any are
left alone because they look suspicious, "For everyone in <inbox>" in a
shared inbox, accept (1) and "Just this one" (Esc). "Just this one" SHALL call
`conversations.dismiss_rule_suggestion`. A later action SHALL replace a
pending offer.

#### Scenario: Teaching once
- **WHEN** the user moves a newsletter to Newsletters and 40 more like it are in the inbox
- **THEN** the toast says "Moved to Newsletters · <subject>" and the offer asks "Move all 40 like this to Newsletters, and new ones too?" with its samples

#### Scenario: Just this one
- **WHEN** the user chooses "Just this one"
- **THEN** the offer closes and the suggestion is turned down for 30 days

### Requirement: Progress and Undo for a rule
Accepting SHALL call `conversations.create_rule` with the chosen likeness and
`apply_to_existing`, push an undo entry (Z) that removes the rule and puts
its mail back, and turn the offer into the run's progress, read from
`conversations.get_rule` and kept current by the change feed: "Finding all
the mail like this" while it counts, then "Moving 1,000 to Updates" with a
progress bar and "340 of 1,000 done", then "Moved 998 to Updates. New ones
will go there too." with how many were left alone as suspicious. Undo all
SHALL call `conversations.remove_rule` with `restore=true` and follow the run
putting the mail back. Close (Esc) SHALL hide it; the run goes on.

#### Scenario: Undo all
- **WHEN** the user accepts, sees "Moved 40 to Newsletters", then chooses Undo all
- **THEN** the rule is removed with its mail put back

### Requirement: Mail a rule handled says so
A conversation row a rule placed or changed SHALL show, in the slot
suggestions use, the Visor and "Campbooks moved this · like <n> others
<likeness>" (or archived, marked read, cleared). The reader's note SHALL say
the same and offer "Stop doing this", which removes the rule without putting
mail back and pushes an undo entry that makes it again.

#### Scenario: A week later
- **WHEN** the user opens a notification a rule moved to Updates
- **THEN** the note says "Campbooks moved this · like 1,012 others from notifications@github.com" and offers Stop doing this

### Requirement: The exception question
When an operation's response names a rule with two or more exceptions in the
last 30 days, the inbox SHALL ask in an offer "You changed back 2 this month.
Keep the rule “<sentence>”?" with Remove the rule (1, when the user may change
it) and Keep it (Esc).

#### Scenario: Twice in a month
- **WHEN** the user moves a second rule-moved notification back to Primary within 30 days
- **THEN** the offer asks whether to keep the rule
