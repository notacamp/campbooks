## ADDED Requirements

### Requirement: The admin module's changes are tracked
Every change to an instance administrator's profile SHALL be tracked as an
instance-wide event of the tracking plan, in the transaction of the change,
with a snapshot of the profile read from the `admin` module's own rows: a
profile granted or given more permissions (`admin.profile_granted`), and
permissions taken away or the whole profile removed
(`admin.profile_revoked`), each with the permissions held before. The
actor SHALL be `system` with the command's name (`admin`). A grant or a
revoke that changes no permission SHALL track nothing. Events SHALL carry
ids, enums, booleans and times only: never an address, a person's name or
the operator's note, and an account id only when it is a Campbooks account
id. Listing the administrators, reading the audit trail and appending to it
SHALL track nothing.

#### Scenario: An operator makes someone an administrator
- **WHEN** an operator grants `system.read` to an account with no profile
- **THEN** one `admin.profile_granted` event is written with no workspace, actor `system` `admin`, `new_profile` true, `before` empty, `granted` `["system_read"]` and the profile's snapshot, which holds the account id and no note
- **AND** granting `system.read` again tracks nothing

#### Scenario: An operator takes a permission away
- **WHEN** an operator revokes `jobs.retry` from a profile holding `jobs.read` and `jobs.retry`
- **THEN** one `admin.profile_revoked` event is written with `before` `["jobs_read", "jobs_retry"]`, `revoked` `["jobs_retry"]` and the profile holding `jobs_read`
- **AND** revoking `jobs.retry` again tracks nothing

#### Scenario: An operator removes an administrator
- **WHEN** an operator revokes every permission of a profile
- **THEN** one `admin.profile_revoked` event is written whose profile has no permissions and a `removed_at`
- **AND** revoking from the account again, now with no profile, tracks nothing

#### Scenario: A profile of another identity provider's account
- **WHEN** an operator grants a permission to an account id that is not a Campbooks account id
- **THEN** the event is written and its snapshot has no `account_id`
