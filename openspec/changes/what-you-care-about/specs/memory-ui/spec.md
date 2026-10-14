## ADDED Requirements

### Requirement: Teach Scout in your own words
Scout's memory SHALL offer, above the rules, a composer, "Teach Scout
something". `Enter` SHALL call `assistant.compile_instruction` and show,
while it runs, the visor thinking and "Only your sentence is sent, never
your mail". For each clarification it SHALL ask the question with its
candidates and a type-ahead; for each proposal it SHALL show the rule
sentence, "You wrote: …", the match preview, the inbox, "Also for the N you
already have", and Save (`Enter`), Change and Cancel (`Esc`). Save SHALL
call `conversations.create_rule` with `state = active`. A note proposal
SHALL be saved to the profile; an unsupported request SHALL be explained
with the closest rule offered. When compile answers `compile_unavailable`,
the composer SHALL be replaced by a manual rule builder (When … Then …)
with the same preview and Save.

#### Scenario: Anything from my accountant
- **WHEN** a member types "anything from my accountant always matters", presses Enter, picks Rita Gomes, and presses Enter
- **THEN** a rule "Always show mail from Rita Gomes (rita@silva-contas.example)" is created with origin `instruction`, and it appears in the rules list quoting the sentence

#### Scenario: Forward is not a rule
- **WHEN** a member types "forward invoices to my accountant"
- **THEN** Scout says it can't forward mail and offers to label mail with "invoice" in the subject instead, and nothing is saved until they choose

#### Scenario: No language model
- **WHEN** compile answers `compile_unavailable`
- **THEN** the manual builder is shown and saving it creates the same kind of rule

### Requirement: About you in Scout's memory
Scout's memory SHALL list, next to the rules, "About you": one sentence
per profile answer and note in the member's locale, each with Change and
Remove (Undo on a toast), plus "Answer the questions again" (the flow with
the current answers selected) and "What Scout reads" (the exact
preferences block and its length out of 600 characters). Rules with origin
`onboarding` SHALL say "From your answers" and rules with origin
`instruction` SHALL quote the member's words.

#### Scenario: Removing an answer
- **WHEN** a member removes "Clients, and invoices and payments, matter most to you"
- **THEN** the profile loses its topics, the block's revision grows, and a toast offers Undo
