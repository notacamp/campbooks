## ADDED Requirements

### Requirement: Still needed?
An item whose need is a still-needed question SHALL be shown in Review as a
question naming the sender and how long ago they wrote, in the user's
language: "Pedro Alves asked 3 weeks ago. Still to do?" for `still_to_do`,
"Pedro Alves wrote 3 weeks ago. Still needs a reply?" for
`still_needs_reply` (the plain question "Did … ask you to do something?" or
"Does … need a reply?" when the sent time is unknown). Its Yes (1) SHALL call
`conversations.confirm_need` and its No (2, X) `conversations.resolve_need`
with `not_needed`; both take the item off the list with Undo, and No SHALL
neither say Campbooks will learn from it nor offer to teach a rule. An item
whose ask lapsed SHALL not be shown.

#### Scenario: An old lease
- **WHEN** Pedro Alves's undated request to sign a lease is three weeks old
- **THEN** Home shows it under Review as "Pedro Alves asked 3 weeks ago. Still to do?" with Yes and No

#### Scenario: Yes
- **WHEN** the user presses 1 on it
- **THEN** the item leaves Review with the toast "Thanks · Pedro Alves stays on your list", Z brings the question back, and it returns under Do as the user's own

#### Scenario: No longer needed
- **WHEN** the user presses 2 on it
- **THEN** the item leaves the list with the toast "No longer needed · Pedro Alves" and no rule is offered
