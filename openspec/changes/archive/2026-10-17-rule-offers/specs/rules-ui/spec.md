## ADDED Requirements

### Requirement: The smart-rule confirmation
The conversations client module SHALL export `SmartRuleConfirm`, a dialog
every offer of a rule goes through before anything is saved (teach once,
the offers after Report and delete and after Already paid, and "Scout
noticed"). It SHALL show the rule as one sentence labelled "Smart rule"; say
that it runs on its own with no AI and lives in Scout's memory → Smart rules,
where it can be changed or turned off; say what it matches from
`conversations.preview_rule` (how many conversations it would change, and up
to three of their subjects as plain text; "None yet" when it would change
none, for a rule about mail still to come); offer "Also apply it to the N
already here" (checked; not shown when there are none); and Save smart rule
(⌘↵) and Cancel (Esc, nothing saved). Saving SHALL call
`conversations.create_rule` (with the suggestion's `notice_id` for "Scout
noticed"), show "Smart rule saved · It's in Scout's memory" for about a
second, and push a toast whose Undo (Z) removes the rule and puts back what
it changed (`remove_rule` with `restore`). A failed save SHALL say so in the
dialog and keep it open. It SHALL work at 375px (a bottom sheet) and by
keyboard alone.

#### Scenario: Teach once
- **WHEN** Ana moves a GitHub notification to Updates and presses 1 on the offer
- **THEN** the confirmation opens with "Smart rule: Mail from notifications@github.com → Updates", 1,000 matches and three subjects, and nothing is saved until Save smart rule

#### Scenario: Undo
- **WHEN** Ana saves the rule with "Also apply it" checked, then presses Z
- **THEN** the rule is removed and the 1,000 conversations go back
