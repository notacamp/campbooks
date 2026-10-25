## ADDED Requirements

### Requirement: The workspaces module's changes are tracked
Every change the `workspaces` module makes SHALL be tracked as an event of
the tracking plan, in the transaction of the change, with a snapshot of the
records it touched read from the module's own rows: workspaces created,
updated, deleted and purged; members joining, changing role, leaving or
being removed, and paused or resumed when their account is suspended or the
suspension lifted; invitations created; signup requests filed, approved and
rejected; signup codes minted, revoked and redeemed. A change that changes
nothing SHALL track nothing. Events SHALL carry ids, enums, booleans, counts
and times only: never a workspace's name, an address, a person's name, a
signup code or its label, a reason, or an invitation token or link; an
invitation SHALL be named by its id and its invitee not at all. Every
mutating operation, handler and job of the module SHALL have a Coverage
row.

#### Scenario: An owner renames a workspace
- **WHEN** an owner changes a workspace's name
- **THEN** one `workspaces.workspace_updated` event is written with actor `user`, operation `workspaces.update`, `fields` `["name"]` and the workspace's snapshot, which holds no name
- **AND** sending the same name again tracks nothing

#### Scenario: Someone is invited and joins
- **WHEN** an admin invites an address and its owner accepts
- **THEN** `workspaces.invitation_created` is written with the invitation's id, role and state `pending`, and `workspaces.member_joined` with `via` `invitation` and the invitation `accepted`; neither holds the address

#### Scenario: A member is removed
- **WHEN** an owner removes a member keeping their mail
- **THEN** one `workspaces.member_left` event is written with `how` `removed`, `mail` `keep` and the membership as it stood

#### Scenario: An account is suspended
- **WHEN** identity suspends an account whose user belongs to two workspaces
- **THEN** one `workspaces.member_suspended` event is written in each, with actor `system`; the same change delivered again tracks nothing

#### Scenario: A deleted workspace is purged
- **WHEN** a deleted workspace's grace period ends and the purge job runs
- **THEN** one `workspaces.workspace_purged` event is written in the purge's transaction
