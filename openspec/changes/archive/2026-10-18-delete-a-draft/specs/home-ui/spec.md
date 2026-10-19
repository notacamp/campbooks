## ADDED Requirements

### Requirement: Deleting the draft from the Reply dialog
In its draft stage the Reply dialog SHALL offer "Delete draft" once a draft
exists: when it opened on a draft whose send was undone, or after Open in
composer saved one. It SHALL NOT be offered before anything is saved. It
SHALL delete that draft, close the dialog and show "Draft deleted" with
Undo, which recreates the draft with the text that was on screen. A failed
delete SHALL keep the dialog and the draft and say so.

#### Scenario: Delete a resumed draft
- **WHEN** the dialog opens on a draft whose send was undone and the user chooses Delete draft
- **THEN** the draft is deleted, the dialog closes and the toast offers Undo

#### Scenario: Nothing to delete yet
- **WHEN** the user has picked one of Scout's replies but not saved a draft
- **THEN** the dialog does not offer Delete draft
