## MODIFIED Requirements

### Requirement: The smart-rule confirmation
The conversations client module SHALL export `SmartRuleConfirm`, a dialog
every offer of a rule goes through before anything is saved (teach once,
the offers after Report and delete and after Already paid, and "Scout
noticed"). For "Scout noticed" it SHALL first ask "What should happen to
<sender>'s mail?" as choice cards: Scout's guess (tagged "Scout's guess";
archive on arrival, or keep off Home and file under the stream), "Highlight
them" (tagged "I like these": Scout makes sure the member sees each new one,
which waits on Home under Worth a look until opened once; nothing is pinned),
"File them under Newsletters" (off Home, still in the inbox; not shown when it
is Scout's guess), "Mark them read" (nothing else changes), and "Or tell Scout
in your words": a sentence read by `assistant.compile_instruction`, whose
actions (the sender stays the one the notice names) become one more card, "As
you said"; when Scout cannot make a rule of it, or has no model, the
confirmation says so and keeps the other cards. Scout's guess SHALL be picked
when the notice's own button opened the confirmation, and nothing when
"Something else…" did: until a goal is picked there SHALL be no preview and
Save smart rule SHALL be disabled. It SHALL show the rule as one sentence labelled "Smart rule"; say
that it runs on its own with no AI and lives in Scout's memory → Smart rules,
where it can be changed or turned off; say what it matches from
`conversations.preview_rule` (how many conversations it would change, and up
to three of their subjects as plain text; "None yet" when it would change
none, for a rule about mail still to come); offer "Also apply it to the N
already here" (checked; not shown when there are none); and Save smart rule
(⌘↵) and Cancel (Esc, nothing saved). Saving SHALL call
`conversations.create_rule` (with the suggestion's `notice_id` for "Scout
noticed", whichever goal was picked: the sentence, what it matches and "Also
apply it to the N already here" all follow the goal), show "Smart rule saved · It's in Scout's memory" for about a
second, and push a toast whose Undo (Z) removes the rule and puts back what
it changed (`remove_rule` with `restore`). A failed save SHALL say so in the
dialog and keep it open. It SHALL work at 375px (a bottom sheet) and by
keyboard alone.

#### Scenario: Teach once
- **WHEN** Ana moves a GitHub notification to Updates and presses 1 on the offer
- **THEN** the confirmation opens with "Smart rule: Mail from notifications@github.com → Updates", 1,000 matches and three subjects, and nothing is saved until Save smart rule

#### Scenario: Something else
- **WHEN** Ana opens Scout noticed's "Something else…" and picks "Highlight them"
- **THEN** the sentence reads "Mail from builds@ci.example.com → kept on Home until you open it", and saving sends `worth_a_look` with the suggestion's `notice_id`

#### Scenario: Scout's guess
- **WHEN** Ana presses the notice's own button ("Archive them…")
- **THEN** the confirmation opens with Scout's guess picked and tagged

#### Scenario: Undo
- **WHEN** Ana saves the rule with "Also apply it" checked, then presses Z
- **THEN** the rule is removed and the 1,000 conversations go back
