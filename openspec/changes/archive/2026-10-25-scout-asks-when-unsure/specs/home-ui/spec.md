## ADDED Requirements

### Requirement: Scout isn't sure
A question about a reply or an action whose need carries the people of its
email (`unsure`) SHALL be shown on Home as a card that says who wrote to whom
and asks what is next: "Ana Costa wrote to Águas do Ribatejo. You're in Cc."
(or "…wrote to Águas do Ribatejo and you." when the inbox is in `To` with
others, and "…and 2 others" past two names), a sub-line that they haven't
answered in 9 days when the headers say none of them wrote since, then "Scout
can't tell whose move this is. What's next for you?", and the chip "Scout
isn't sure". Its own actions SHALL be on 1 to 3, in one style: "Nudge {first
name}…" (the existing nudge dialog, over the conversation that now waits on
that person; hidden when there is no obvious party, and for an inbox the
member may only read), "Reply to {first name}…" (the existing reply dialog,
after Yes is filed) and "Nothing for me" (Not needed); then Later (S) and Ask
Scout (`.`). Choosing Nudge SHALL file "someone else has the move" and fold the
row with a one-line confirmation and Undo (Z); closing the dialog without
sending SHALL leave the conversation waiting. The disclosure "How Scout
decided" SHALL show the model's confidence, the sure bar and, when it applied,
"You're in Cc, not To.". A question whose email went to the inbox alone SHALL
stay today's Yes / No card. At 375px the buttons SHALL stack at full width with
targets of at least 44px and the card SHALL not scroll sideways; its words
SHALL fit in every language.

#### Scenario: Copied on a colleague's mail
- **WHEN** Home lists a question about João's message to Ana Costa that copies the inbox, unanswered for 9 days
- **THEN** the row reads "João Pereira wrote to Ana Costa. You're in Cc.", "They haven't answered in 9 days. Scout can't tell whose move this is. What's next for you?", the chip "Scout isn't sure", and the buttons Nudge Ana…, Reply to João… and Nothing for me

#### Scenario: Nudge
- **WHEN** the user presses 1
- **THEN** the row folds with "Waiting on Ana Costa" and Undo, and the nudge dialog opens for Ana

#### Scenario: Written to the inbox alone
- **WHEN** the question's email went to the inbox only
- **THEN** the row has today's Yes and No
