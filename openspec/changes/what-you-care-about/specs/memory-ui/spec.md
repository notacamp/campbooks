## Purpose

Scout's memory: the page where a member sees what Scout was taught,
teaches it more in their own words, and answers the questions again.

## ADDED Requirements

### Requirement: The Scout's memory page
Settings SHALL have a Scout's memory page at
`/w/{workspace_id}/settings/memory` with, in order: "Teach Scout
something", the rules (a slot the conversations module fills), and "Answer
the questions" (the question flow). On phones it SHALL be one column with
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
you already have", Save (`Enter`) and Cancel (`Esc`). For `unsupported` it SHALL say what Scout
can't do, in plain words for the reason, and for `unclear` how to say it
as a rule; any proposal left SHALL be offered as the closest rule.

#### Scenario: Anything from my accountant
- **WHEN** a member types "anything from my accountant always matters", presses Enter and picks Rita Gomes
- **THEN** the proposal reads "Mail from Rita Gomes (rita@silva-contas.example) always stays in your inbox and never gets filed away.", quotes what they wrote, shows how many emails it matches, and Save creates it with origin `instruction` and their words

#### Scenario: Forwarding
- **WHEN** the answer is `unsupported` with `sends_mail`
- **THEN** the page says Scout can't send or forward mail, and nothing is saved

### Requirement: A rule builder without AI
When compile answers 503 (`ai_role_not_configured` or
`ai_external_content_refused`), the composer SHALL say that this server
has no language model for sentences and show a builder instead: a
condition (the sender is, the sender's domain is, the subject contains,
it's from a mailing list, it's an automatic notification) and an action
(always show it, never needs me, move to a stream, archive), producing the
same proposal card.

#### Scenario: No model
- **WHEN** compile answers `ai_role_not_configured`
- **THEN** the builder is shown, and choosing "the sender is" rita@silva-contas.example with "always show it" shows the same proposal as the composer would
