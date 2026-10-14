## ADDED Requirements

### Requirement: Removing a member decides their mail
`DELETE …/members/{user_id}` SHALL take `mail=delete` (the default) or
`mail=keep`. `workspaces.member_left` SHALL carry the choice. Keeping
SHALL be refused with 409 `keep_needs_a_reader` when an inbox the member
connected would have nobody left to read it, asked in the departure's own
transaction. A member who leaves on their own SHALL always have their mail
deleted. Accepting an invitation SHALL be refused with 409 `still_leaving`
while a departure of that person from the workspace is not yet handled by
every module.

#### Scenario: Offboarding with history kept
- **WHEN** an owner removes a member with `mail=keep`
- **THEN** `workspaces.member_left` says `"mail": "keep"`

#### Scenario: Keeping mail nobody could read
- **WHEN** an owner removes, with `mail=keep`, a member who is the only member of an inbox they connected
- **THEN** the answer is 409 `keep_needs_a_reader` and the member stays
