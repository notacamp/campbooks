## MODIFIED Requirements

### Requirement: A suspension reaches every workspace of the account
On `identity.account_suspended` and `identity.account_unsuspended` the module
SHALL find the user whose identity subject is the account, lock their row,
and, unless the change is older than the one last announced for them, record
it on that row and publish, in each workspace they are a member of that is
not being deleted, `workspaces.member_suspended` or
`workspaces.member_unsuspended` with the workspace, the user, the event's
`seq` (when it has one) and its `at` (its `occurred_at` when it has none),
each in a transaction working in that workspace, keeping the user's row
locked until it is done. A change is older when its `seq` is lower, when it
has no `seq` and the one recorded has, or, neither having one, when its `at`
is earlier; the same change again SHALL be announced again (consumers ignore
it). Accepting an invitation, creating a workspace, and leaving or being
removed SHALL lock the user's row before any other lock; accepting SHALL then
announce the recorded change, suspension or lift, in the workspace joined,
and creating SHALL announce a recorded suspension in the new workspace. An
account with no user, or a user in no workspace, SHALL publish nothing.

#### Scenario: A member of two workspaces
- **WHEN** an account whose user belongs to two workspaces is suspended
- **THEN** `workspaces.member_suspended` is published once in each, with that user and the suspension's `seq` and `at`, and nothing for another user

#### Scenario: Never signed in
- **WHEN** an account that never signed in (no user) is suspended
- **THEN** nothing is published

#### Scenario: The clock stepped back
- **WHEN** a suspension with `seq` 1 is followed by its lift with `seq` 2 but an earlier `at`, then the suspension is delivered again, then an event without `seq`
- **THEN** the lift is announced, the late suspension and the event without `seq` are not, and the user's recorded change is the lift

#### Scenario: Joining while being suspended
- **WHEN** a member accepts an invitation to a workspace while their suspension is being announced
- **THEN** the acceptance waits for the announcement, which does not list that workspace, and then announces the suspension there

#### Scenario: Removed while a lift is announced
- **WHEN** a manager removes a member while the member's lift is being announced
- **THEN** the removal waits, and the workspace hears the lift before `workspaces.member_left`

#### Scenario: Back after a lift that missed the workspace
- **WHEN** a suspended member is removed, the suspension is lifted, and they accept a new invitation to that workspace
- **THEN** joining announces the lift there; a workspace they create afterwards hears nothing
