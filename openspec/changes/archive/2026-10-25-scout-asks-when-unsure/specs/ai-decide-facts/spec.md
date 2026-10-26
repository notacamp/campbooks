## ADDED Requirements

### Requirement: Whether the inbox is addressed
The facts SHALL say, from the headers' addresses only and without regard to
case, whether one of the inbox's addresses is in `To` of a message
(`addressed_to_inbox`): `true`, `false` for mail the inbox only got a copy of
or was not named in, and unknown when the inbox is. The policy SHALL use it
only to lower trust: a model's `needs_reply` or `asks_action` yes for a
message that does not address the inbox SHALL be a question, not a `sure`
need.

#### Scenario: In Cc
- **WHEN** the inbox is in `Cc` and not in `To`
- **THEN** the fact is false

#### Scenario: In To under another case
- **WHEN** the inbox is sam@example.com and `To` has Sam@Example.com
- **THEN** the fact is true

#### Scenario: No inbox known
- **WHEN** the envelope names no recipient
- **THEN** the fact is unknown and trust is not lowered
