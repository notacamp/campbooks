## ADDED Requirements

### Requirement: A member's signature per inbox
A member SHALL be able to read and set their own signature for an inbox
they may send from, with `conversations.get_signature` and
`conversations.set_signature`. A signature SHALL be text with the marks a
message may carry, at most 2,000 characters; longer text SHALL answer 400
`signature_too_long`. A member SHALL never see or change another member's
signature. A `reader` of the inbox SHALL get 403 `inbox_read_only`, and the
assistant SHALL NOT be able to call either operation.

#### Scenario: Set and read
- **WHEN** Sam sets the signature "Sam Rivers · Rivers Lettings" for the office inbox and reads it back
- **THEN** it reads "Sam Rivers · Rivers Lettings"

#### Scenario: A shared inbox
- **WHEN** Sam and Ana both send from office@ and Ana sets her signature
- **THEN** Sam's signature for office@ is unchanged

### Requirement: Drafts carry the signature they show
A new draft SHALL take its author's signature for its inbox when it is
created. A save SHALL be able to leave the signature off, or add the
author's current one back. Editing a signature SHALL NOT change drafts
already started. On send, the signature SHALL follow the member's text: in
the plain-text part after a `-- ` line, and in the HTML part in its own
block before the quoted history.

#### Scenario: A reply signed
- **WHEN** Sam, whose signature for the inbox is "Sam Rivers", starts a reply and sends it
- **THEN** the plain-text part reads Sam's text, then `-- ` and "Sam Rivers" on its own lines, then the quoted history

#### Scenario: Left off
- **WHEN** Sam leaves the signature off a reply and sends it
- **THEN** the message carries no signature

#### Scenario: Edited meanwhile
- **WHEN** Sam starts a reply, then changes their signature in Settings, then sends the reply
- **THEN** the reply carries the signature it showed when it was started
