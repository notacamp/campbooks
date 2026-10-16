## Purpose

Scout's memory: the page where a member sees what Scout was taught,
teaches it more in their own words, and answers the questions again.

## ADDED Requirements

### Requirement: The Scout's memory page
Settings SHALL list Scout's memory as one of its sections, a page at
`/w/{workspace_id}/settings/memory` with, in order: "Teach Scout
something", the rules (the conversations module's list, `rules-ui`), and
"Answer the questions" (the question flow, `Q`). While the member's answers
load it SHALL show a placeholder, and when they fail to load it SHALL say
so with Try again, never "you haven't answered". On phones it SHALL be one column with
no horizontal scrolling at 375px, in en, pt-PT, es-ES and fr.

#### Scenario: Opening it
- **WHEN** a member opens Scout's memory
- **THEN** the composer is first, then the rules, then the way back to the questions

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
as a rule; any proposal left SHALL be offered as the closest rule.

#### Scenario: Anything from my accountant
- **WHEN** a member types "anything from my accountant always matters", presses Enter and picks Rita Gomes
- **THEN** the proposal reads "Mail from Rita Gomes (rita@silva-contas.example) always stays in your inbox and never gets filed away.", quotes what they wrote, shows how many emails it matches, and Save creates it with origin `instruction` and their words

#### Scenario: Forwarding
- **WHEN** the answer is `unsupported` with `sends_mail`
- **THEN** the page says Scout can't send or forward mail, and nothing is saved

### Requirement: A rule builder without AI
When compile answers 409 `compile_unavailable`, the composer SHALL say that this server
has no language model for sentences and show a builder instead: a
condition (the sender is, the sender's domain is, the subject contains,
it's from a mailing list, it's an automatic notification) and an action
(always show it, never needs me, move to a stream, archive), producing the
same proposal card. After an ordinary compile failure, "Show the rule"
SHALL open the same builder saying Scout couldn't read the sentence, never
that there is no model. While the inboxes load the builder SHALL say so,
never that they are read-only.

#### Scenario: No model
- **WHEN** compile answers `compile_unavailable`
- **THEN** the builder is shown, and choosing "the sender is" rita@silva-contas.example with "always show it" shows the same proposal as the composer would
