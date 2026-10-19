## ADDED Requirements

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

## MODIFIED Requirements

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
