## ADDED Requirements

### Requirement: A suspension reaches every workspace of the account
On `identity.account_suspended` and `identity.account_unsuspended` the module
SHALL find the user whose identity subject is the account and publish, in
each workspace they are a member of that is not being deleted,
`workspaces.member_suspended` or `workspaces.member_unsuspended` with the
workspace, the user and the event's `at` (its `occurred_at` when it has
none), each in a transaction working in that workspace. An account with no
user, or a user in no workspace, SHALL publish nothing. Delivering an event
again MAY publish again; consumers SHALL treat the newest `at` as the truth.

#### Scenario: A member of two workspaces
- **WHEN** an account whose user belongs to two workspaces is suspended
- **THEN** `workspaces.member_suspended` is published once in each, with that user and the suspension's `at`, and nothing for another user

#### Scenario: Never signed in
- **WHEN** an account that never signed in (no user) is suspended
- **THEN** nothing is published
