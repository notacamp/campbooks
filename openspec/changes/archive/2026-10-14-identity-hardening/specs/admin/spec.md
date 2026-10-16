## MODIFIED Requirements

### Requirement: Admin API authorisation
Admin operations SHALL be served under `/admin/v1/<module>/` and SHALL
require a user (not client-credentials) bearer token carrying the scope
`admin:access` and an admin profile holding the operation's permission;
otherwise they SHALL answer 404, indistinguishable from an unknown route. A
token bound to a workspace SHALL also answer 404. Once those pass, a token
whose `auth_time` is more than one hour old SHALL be refused with 401 and the
problem code `admin_reauth_required`. The issuer SHALL grant `admin:access`
only to first-party clients.

#### Scenario: Not an administrator
- **WHEN** a signed-in user without a profile calls `GET /admin/v1/system/overview`
- **THEN** the response is 404 and nothing is recorded in the audit trail

#### Scenario: Third-party client
- **WHEN** an OAuth client that is not first-party requests the scope `admin:access`
- **THEN** the issuer does not grant it, and an admin request with that client's token answers 404

#### Scenario: Bound token
- **WHEN** a super admin's token bound to a workspace calls an admin operation
- **THEN** the response is 404

#### Scenario: Stale authentication
- **WHEN** a super admin's token has `auth_time` two hours ago
- **THEN** admin requests answer 401 with code `admin_reauth_required`, and after signing in again they succeed
