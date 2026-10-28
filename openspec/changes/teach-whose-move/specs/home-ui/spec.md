## ADDED Requirements

### Requirement: The wait rule offer
After "Nudge {party}…" on Scout isn't sure, once the server agreed, Home SHALL ask
`conversations.suggest_rules` with `action=waiting` and, when there is an
offer, show it docked above the toasts: "When Ana writes to someone else with
you in Cc, should Scout wait on them for you?", the sub-line "Scout would wait
on them and suggest a nudge after 3 working days. It never sends anything on
its own." (the days from the offer), "Yes, always" (1) and "Ask me each time"
(Esc, which dismisses the offer for that sender). Yes SHALL open the smart-rule
confirmation and never save on its own. There is an offer only from the second
time the member gives that same answer for the same sender; Undo of the
answer SHALL cancel an offer not yet shown or shown. The offer SHALL fit in
every language and stack at full width at 375px. Nothing is sent on the rule's
behalf.

#### Scenario: The second time
- **WHEN** the member presses 1 (Nudge Ana…) on Ana's second such mail
- **THEN** the nudge dialog opens and the offer "When Ana writes to someone else with you in Cc, should Scout wait on them for you?" appears with Yes, always and Ask me each time

#### Scenario: The first time
- **WHEN** it is the first such answer for that sender
- **THEN** no offer is shown

#### Scenario: Ask me each time
- **WHEN** the member presses Esc on the offer
- **THEN** the offer is dismissed through `dismiss_rule_suggestion` and is not shown again for that sender for 30 days, and never after the second dismissal
