## ADDED Requirements

### Requirement: Telling Scout about the person in the reply dialog
Under the quoted message, the reply dialog SHALL show one quiet line about
the person answered, found as the server finds them (the sender of a
message received, the first recipient of one sent): "Tell Scout about
<first name>" (marked with the visor) when the member has no note about
their address, or "Scout knows: <the note, on one line, cut with an
ellipsis>" with "Edit" when they have one. The line SHALL be hidden when the
address is not plain, while the member's notes load or fail to load, and
when the server has no model for replies (`reply_unavailable`), since the
note only steers Scout. The client SHALL NOT try to tell a distrusted
conversation apart: the server gives a distrusted message no note.

Activating the line SHALL open, in the dialog and never as a dialog of its
own, one field labelled "About <name>, and how to write to them" with the
placeholder "e.g. My landlord. Formal, in Portuguese.", at most 500
characters with a counter near the limit, Save (⌘↵ / Ctrl+Enter, shown as
the app shows its hints) and Cancel (Esc), and, for a note kept, "Forget
this". ⌘↵ and Esc in the field SHALL save and cancel the note only: they
SHALL never send the reply or close the dialog. Save SHALL call
`conversations.set_person_note` with the address in the body, show the new
line, put the focus back on it, announce "Saved" in a live region, and say
so in a toast with Undo that puts back what was there before. Forget this
SHALL call `conversations.delete_person_note`, with Undo. Once a note is
saved while a draft shows, the dialog SHALL offer "Rewrite with this", which
calls `conversations.write_reply` again with the same intent; it SHALL never
rewrite on its own. A note that can't be kept SHALL say why under the field
(`person_notes_full`, `person_note_invalid`, or that it failed) and keep the
words. In en, pt-PT, es-ES and fr, and at 375px with no horizontal
scrolling.

#### Scenario: Tell Scout about Marta
- **WHEN** Sam opens Reply on Marta's email, chooses "Tell Scout about Marta", types "My landlord. Formal, in Portuguese." and presses ⌘↵
- **THEN** the note is kept for marta's address, the line reads "Scout knows: My landlord. Formal, in Portuguese." with Edit, the focus is on it, and nothing is sent

#### Scenario: Rewrite with this
- **WHEN** a draft shows and Sam saves a note about Marta
- **THEN** "Rewrite with this" is offered, and Scout writes the draft again only when Sam chooses it

#### Scenario: Esc keeps the dialog
- **WHEN** Sam presses Esc in the note's field
- **THEN** the field closes with nothing kept, the dialog stays open, and the focus is back on the line

#### Scenario: No model on this server
- **WHEN** the server answers `reply_unavailable`
- **THEN** the dialog shows no line about the person
