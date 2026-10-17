## MODIFIED Requirements

### Requirement: Suspending an account
The `identity` module SHALL serve `POST /admin/v1/identity/accounts/{account_id}/suspend`
and `…/unsuspend` (`users.suspend`). Suspending SHALL, in one transaction,
mark the account suspended, end its issuer sessions, revoke its refresh-token
families, record `account_suspended` (with the administrator's account id)
in the account's audit trail and publish `identity.account_suspended`; the
web BFF SHALL then delete the account's web sessions. Unsuspending SHALL, in
one transaction, lift the suspension, record `account_unsuspended` and
publish `identity.account_unsuspended`. Both events SHALL carry `at`, read
after the account's row is locked, so a later change to one account always
carries a later `at`. Both operations SHALL be idempotent: repeating one
changes, records and publishes nothing more and answers the account as it
is. Suspending an account that has an admin profile SHALL be refused with
409 `account_is_admin`, read through the kernel's `AdminProfiles` port, and
change nothing.

#### Scenario: Suspend once, retry safely
- **WHEN** a super admin suspends an account twice
- **THEN** both answers are 200 with the same `suspended_at`, the account has no live session or refresh family, and one audit row and one event exist

#### Scenario: Suspend, then lift
- **WHEN** a super admin suspends an account and then unsuspends it twice
- **THEN** one `identity.account_unsuspended` is published, with an `at` later than the suspension's

#### Scenario: An administrator cannot be suspended over HTTP
- **WHEN** a super admin suspends an account that has an admin profile, asking in Portuguese
- **THEN** the answer is 409 `account_is_admin` whose detail tells, in Portuguese, to revoke the profile with the server's command line first, and the account is not suspended

#### Scenario: Web sessions end at once
- **WHEN** `identity.account_suspended` is delivered for an account signed in on two browsers
- **THEN** both browsers' web sessions are gone and another account's are not
