# memory-ui Specification

## MODIFIED Requirements

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
