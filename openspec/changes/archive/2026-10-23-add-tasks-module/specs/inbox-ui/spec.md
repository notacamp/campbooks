## ADDED Requirements

### Requirement: Make it a task
The reader SHALL offer "Make it a task" with `n t` (the same "new task" key, here with the email as its source), which opens an inline form
(not a dialog) with the subject as the title, the due chips and Do/Pay. Add
SHALL call `tasks.create` with the conversation as the source, close the form,
and show "Added to your tasks" with Undo and Open.

#### Scenario: From the lease email
- **WHEN** Ana presses `n t` on "Lease for Rua das Flores 12", edits the title to "Send the signed lease to Ana", picks Fri and adds it
- **THEN** a task due Friday with that conversation as its source is created, and Tasks shows "From Ana Ribeiro · email" on it
