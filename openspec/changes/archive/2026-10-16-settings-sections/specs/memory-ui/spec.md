# memory-ui Specification

## MODIFIED Requirements

### Requirement: The Scout's memory page
Settings SHALL list Scout's memory as one of its sections, a page at
`/w/{workspace_id}/settings/memory` made of section cards (`settings-ui`)
with, in order: "Teach Scout something", "Your rules" (the conversations
module's list, `rules-ui`, which SHALL NOT repeat a heading of its own, and
whose empty state reads "No rules yet") and "Your answers" ("Answer the
questions", the question flow, `Q`). The sentence field's label SHALL be
available to screen readers only, and the line saying only the sentence is
sent SHALL carry a lock. The answer SHALL read as a "When Scout is sure"
label and its value. While the member's answers load it SHALL show a
placeholder, and when they fail to load it SHALL say so with Try again, never
"you haven't answered". On phones it SHALL be one column with no horizontal
scrolling at 375px, in en, pt-PT, es-ES and fr.

#### Scenario: Opening it
- **WHEN** a member opens Scout's memory
- **THEN** the composer card is first, then the rules card, then the answers card with the way back to the questions
