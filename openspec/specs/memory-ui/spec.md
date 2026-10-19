# memory-ui Specification

## Purpose
Scout's memory: the page where a member sees what Scout was taught,
teaches it more in their own words, and answers the questions again.

## Requirements

### Requirement: The Scout's memory page
Settings SHALL list Scout's memory as one of its sections, a page at
`/w/{workspace_id}/settings/memory` made of section cards (`settings-ui`)
with, in order: "Teach Scout something", "Your rules" (the conversations
module's list, `rules-ui`, which SHALL NOT repeat a heading of its own, and
whose empty state reads "No rules yet"), "People you told Scout about" (the
conversations module's notes, which SHALL NOT repeat a heading of its own)
and "Your answers" ("Answer the
questions", the question flow, `Q`). The sentence field's label SHALL be
available to screen readers only, and the line saying only the sentence is
sent SHALL carry a lock. The answer SHALL read as a "When Scout is sure"
label and its value. While the member's answers load it SHALL show a
placeholder, and when they fail to load it SHALL say so with Try again, never
"you haven't answered". On phones it SHALL be one column with no horizontal
scrolling at 375px, in en, pt-PT, es-ES and fr.

#### Scenario: Opening it
- **WHEN** a member opens Scout's memory
- **THEN** the composer card is first, then the rules card, then the people card, then the answers card with the way back to the questions

### Requirement: Teach Scout in your own words
`Enter` in the composer SHALL call `assistant.compile_instruction` and show,
while it runs, the visor thinking and "Only your sentence is sent, never
your mail". For a proposal with a `who`, it SHALL ask "Who do you mean by
…?" with the candidates and a field for an address or domain; for each
proposal it SHALL show the rule sentence, "You wrote: …", the inbox, and
the match preview (`conversations.preview_rule`), "Also apply to the mail
you already have", Save (`⌘↵` / `Ctrl+Enter`) and Cancel (`Esc`). The focus
SHALL stay in the composer when a proposal appears, and a live region
SHALL announce the rule understood; only when nothing holds the focus (the
builder's form is gone) SHALL it move to Save. When a rule saves in some
inboxes and fails in others, Save SHALL become Try again and send only the
inboxes still missing it, with the same teaching. For `unsupported` it SHALL say what Scout
can't do, in plain words for the reason, and for `unclear` how to say it
as a rule; any proposal left SHALL be offered as the closest rule, and both
SHALL offer "Build a rule yourself".

#### Scenario: Anything from my accountant
- **WHEN** a member types "anything from my accountant always matters", presses Enter and picks Rita Gomes
- **THEN** the proposal reads "Mail from Rita Gomes (rita@silva-contas.example) always stays in your inbox and never gets filed away.", quotes what they wrote, shows how many emails it matches, and Save creates it with origin `instruction` and their words

#### Scenario: Forwarding
- **WHEN** the answer is `unsupported` with `sends_mail`
- **THEN** the page says Scout can't send or forward mail, and nothing is saved

#### Scenario: Unclear
- **WHEN** the answer is `unclear`
- **THEN** the page says how to say it as a rule and offers "Build a rule yourself"

### Requirement: A rule builder without AI
Next to the privacy line, "Build a rule yourself" SHALL open a rule builder
at any time, calling no model, saying "Pick which mail, then what Scout does
with it. No AI reads this."; while a language model may read sentences, the
builder SHALL offer "Tell Scout in a sentence instead". When compile answers
409 `compile_unavailable`, the composer SHALL say that this server has no
language model for sentences and show the builder instead, with no way back.
After an ordinary compile failure, "Show the rule" SHALL open the same builder
saying Scout couldn't read the sentence, never that there is no model. The
builder SHALL have a condition (it's from, the subject contains, it's from a
mailing list, it's an automatic notification) and an action (always show it,
never needs me, move to a stream, archive). "It's from" SHALL find senders by
name with the same filter box as Scout's questions (`conversations.list_senders`;
a full address or a domain no one matches as its own card; a free mail
domain refused, saying why), and each ticked sender SHALL become its own rule
(its address, or anyone at its domain), since all of a rule's conditions must
hold. Each rule SHALL get the same proposal card as the composer's, each with
its own Save, and once every card is saved or cancelled the builder SHALL be
back, empty. With no sender ticked it SHALL say to tick one. While the inboxes
load the builder SHALL say so, never that they are read-only.

#### Scenario: No model
- **WHEN** compile answers `compile_unavailable`
- **THEN** the builder is shown, and choosing "it's from" rita@silva-contas.example with "always show it" shows the same proposal as the composer would

#### Scenario: Building by hand with a model
- **WHEN** a member presses "Build a rule yourself"
- **THEN** the builder opens, no model is called, and "Tell Scout in a sentence instead" brings the composer back

#### Scenario: Two senders
- **WHEN** a member types "tax", ticks Joana Reis, types atelier-conseils.example, ticks it, keeps "always show it" and presses Show the rules
- **THEN** two rule cards show, one for joana@northtax.example and one for anyone at atelier-conseils.example, and saving both creates two rules

### Requirement: People you told Scout about
Scout's memory SHALL have a card "People you told Scout about", listing the
member's own notes (`conversations.list_person_notes`), the newest change
first: each by the person's name, or their address when no name is known,
with the address under the name, and the note. Each SHALL offer Edit, which
opens the same field as the reply dialog's in place (label "About <name>,
and how to write to them", Save with ⌘↵ / Ctrl+Enter, Cancel with Esc,
at most 500 characters with a counter near the limit), and Remove, which
SHALL say so in a toast with Undo that keeps the same note again. After
Save or Cancel the focus SHALL be back on Edit, and a live region SHALL
announce "Saved". With no notes it SHALL read "When you reply to someone,
you can tell Scout who they are and how to write to them." While the notes
load it SHALL show a placeholder; when they fail to load it SHALL say so
with Try again. At 375px it SHALL be one column with no horizontal
scrolling, in en, pt-PT, es-ES and fr.

#### Scenario: Removing Marta's note
- **WHEN** Sam removes the note about Marta
- **THEN** it leaves the list, a toast says Scout forgot what Sam said about her, and Undo brings the same note back

#### Scenario: Editing in place
- **WHEN** Sam edits the note about Marta and presses ⌘↵
- **THEN** the note is kept, the row shows the new words, and the focus is back on Edit

#### Scenario: No notes yet
- **WHEN** Sam never told Scout about anyone
- **THEN** the card says that when he replies to someone, he can tell Scout who they are and how to write to them
