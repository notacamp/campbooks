## ADDED Requirements

### Requirement: What you care about at first run
After the first-run mailbox step (or after the ready view when there is no
mailbox), the client SHALL offer "Tell Scout what you care about" with
Start (`Enter`) and Later (`Esc`), then the assistant question flow. Later
SHALL open Home without saving anything but `asked_at`. Without a mailbox,
the sender views SHALL say rules apply as mail arrives instead of showing
counts.

#### Scenario: Straight to Home
- **WHEN** a new member presses Esc on the offer
- **THEN** Home opens and offers the flow again for 7 days

#### Scenario: No mailbox yet
- **WHEN** a member with no inbox reaches the sender view
- **THEN** it shows no counts and says the rule will apply to mail as it arrives
