## ADDED Requirements

### Requirement: Removing a member decides their mail
`DELETE …/members/{user_id}` SHALL take `mail=delete` (the default) or
`mail=keep`. `workspaces.member_left` SHALL carry the choice. A member who
leaves on their own SHALL always have their mail deleted.

#### Scenario: Offboarding with history kept
- **WHEN** an owner removes a member with `mail=keep`
- **THEN** `workspaces.member_left` says `"mail": "keep"`
