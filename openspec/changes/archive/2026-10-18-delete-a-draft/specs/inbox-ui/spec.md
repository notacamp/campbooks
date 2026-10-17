## ADDED Requirements

### Requirement: Deleting a draft
While the composer is closed, the reader SHALL list every draft the member
can still edit on the conversation (state draft or failed), each as a chip
that names it ("Draft saved · Reply to <name>", "Draft saved · Reply all",
"Draft saved · Forward") and reopens it, with its own "Delete draft" beside
it. Deleting SHALL call `conversations.delete_draft`, remove the chip at once
and show a "Draft deleted" toast with Undo. Undo SHALL create the draft again
with the same kind, message, recipients, subject, text and quoted history; a
forward's attachments come back as a new forward carries them. The
composer's Discard SHALL do the same, restoring what was on screen, and
SHALL never save into the draft it deletes. When the server refuses a
delete, nothing SHALL change and the toast SHALL say so.

#### Scenario: Delete from the chip
- **WHEN** the member presses Esc on a reply draft, then Delete draft beside "Draft saved · Reply to Maya Okafor"
- **THEN** the draft is deleted, the chip goes, and the toast says "Draft deleted" with Undo

#### Scenario: Two drafts
- **WHEN** a conversation has a reply draft and a forward draft and the composer is closed
- **THEN** both are listed by name, and each can be deleted without touching the other

#### Scenario: Undo
- **WHEN** the member presses Undo on "Draft deleted"
- **THEN** a draft with the same recipients, subject, text and quoted history is back and listed

#### Scenario: Discard is undoable
- **WHEN** the member types in a draft, then chooses Discard
- **THEN** the draft is deleted, and Undo brings it back with the text that was typed
