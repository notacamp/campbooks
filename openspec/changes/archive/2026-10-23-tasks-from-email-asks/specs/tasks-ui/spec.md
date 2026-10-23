## MODIFIED Requirements

### Requirement: Where a task came from
A task's detail SHALL show, for a task with a source conversation, the
sender, subject and date of that conversation and an "Open the email" link,
read as the user, and for a task of origin `ask` also that Scout found it in
that email; an own task with no source SHALL show nothing there.

#### Scenario: The lease
- **WHEN** Ana opens the task she made from "Lease for Rua das Flores 12"
- **THEN** the detail shows Ana Ribeiro, that subject, its date and "Open the email"

#### Scenario: Found by Scout
- **WHEN** Ana opens the task Scout made from EDP's invoice
- **THEN** the detail says Scout found it in that email, above EDP's subject and date
