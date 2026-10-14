## ADDED Requirements

### Requirement: Offering to do the same to lookalikes
After the user moves, archives (E), labels, marks read (U) or marks Not needed
(X) one conversation, the inbox SHALL ask `conversations.suggest_rules` and,
when there is a suggestion, show an offer docked above the toast (a sheet
below 48rem) with the Visor, the question in plain words ("Move all 1,000 like
this to Updates, and new ones too?"), the likeness choices with their counts,
five sample subjects with dates, "Also mark them read" where it applies, and
two actions: accept (Y) and "Just this one" (Esc). In a shared inbox it SHALL
say it is for everyone in that inbox. The offer SHALL not block the list:
J, K and E SHALL keep working and dismiss it.

#### Scenario: Teaching once
- **WHEN** the user presses M then 2 on a GitHub notification with 1,000 lookalikes in Primary
- **THEN** the toast says "Moved to Updates · <subject>" and the offer asks "Move all 1,000 like this to Updates, and new ones too?" with "Everything from notifications@github.com" chosen

#### Scenario: Just this one
- **WHEN** the user presses Esc on the offer
- **THEN** it closes, `conversations.dismiss_rule_suggestion` is called, and the single move keeps its Undo

### Requirement: Progress and Undo for a rule
Accepting SHALL call `conversations.create_rule` with `apply_to_existing`
and turn the offer into the run's progress from `conversations.rule_changed`
("Moving 1,000 to Updates", "340 of 1,000 done"), then the result ("Moved 998
to Updates. New ones will go there too.", and how many were left alone as
suspicious) with Undo all (Z) and a link to Scout's memory. Undo SHALL call
`conversations.remove_rule` with `restore=true`. While a run is going the list
SHALL refetch at most once per progress update.

#### Scenario: Undo all
- **WHEN** the user presses Y on the offer, waits for "Moved 998 to Updates", then presses Z
- **THEN** the rule is removed and the 998 conversations come back to Primary

### Requirement: Mail a rule handled says so
A conversation row a rule placed or changed SHALL show a line with the Visor,
"Visor moved this · like <count> others from <likeness>" (or the archive,
label, read wording), in the slot suggestions use. The reader's note SHALL
say what the rule did and how many it has handled, with Move back (M) and
"Stop doing this", and when it was taught.

#### Scenario: A week later
- **WHEN** the user opens a GitHub notification a rule moved to Updates
- **THEN** the note says "Visor moved this to Updates, like 1,012 others from notifications@github.com" and offers Move back and Stop doing this

### Requirement: The exception question
When the user changes back what a rule did, the toast SHALL say the rule will
leave that conversation alone, with Undo. When the response reports a second
exception within 30 days, an alert toast SHALL ask whether to keep the rule,
offering a narrower likeness that leaves out both (1), Remove the rule (2) and
Keep it (Esc).

#### Scenario: Twice in a month
- **WHEN** the user moves a second rule-moved GitHub notification back to Primary within 30 days
- **THEN** the alert asks "You moved 2 back this month. Keep moving all mail from notifications@github.com to Updates?"
