## ADDED Requirements

### Requirement: The signature in the composer
The composer SHALL show the draft's signature under the text, muted and not
editable there, with "Leave off" and "Edit in Settings". When it is left
off and the member has a signature, it SHALL offer "Add signature". With no
signature for that mailbox, it SHALL offer "Add a signature in Settings".

#### Scenario: Leave it off
- **WHEN** the user presses "Leave off" under the signature
- **THEN** the signature disappears, "Add signature" shows instead, and the sent message carries none

#### Scenario: No signature yet
- **WHEN** the user has no signature for the mailbox and opens a reply
- **THEN** the composer shows "Add a signature in Settings" under the text
