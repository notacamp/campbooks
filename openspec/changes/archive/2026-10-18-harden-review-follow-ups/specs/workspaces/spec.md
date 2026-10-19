## MODIFIED Requirements

### Requirement: Runtime database login cannot bypass row-level security
The server SHALL run requests and jobs on a database login that is not a
superuser, does not have `BYPASSRLS`, is not a member of such a role and owns
no table with row-level security, and SHALL apply migrations on a separate
connection (`CAMPBOOKS_DATABASE_MIGRATION_URL` when set). A query on the
runtime login SHALL NOT be able to become a privileged role or see another
workspace's rows. At startup the server SHALL refuse to serve, naming the
reason, when the role it queries as can bypass row-level security, owns
policy tables or is a member of `campbooks_definer` (the owner of the
`SECURITY DEFINER` functions), unless `CAMPBOOKS_DATABASE_ALLOW_RLS_BYPASS`
is set. It SHALL refuse a PostgreSQL older than 15, naming both versions,
before migrating. The migration connection SHALL wait at most a minute for a
lock where a migration sets no limit of its own, after taking the migration
lock: behind a long-running transaction the migration fails, and the server
migrates again at its next start.

#### Scenario: Dedicated login
- **WHEN** the runtime login is a plain member of `campbooks_app` and a query runs `SET ROLE` to a superuser or `RESET ROLE`
- **THEN** the first is refused, the second leaves the session on the same unprivileged login, and a workspace transaction sees only its own workspace's rows

#### Scenario: Privileged login
- **WHEN** the runtime URL is a superuser and no migration URL is set
- **THEN** the connections switch to `campbooks_app` and the server starts with a warning recommending separate logins

#### Scenario: Refusal
- **WHEN** the runtime role is a superuser (for example `CAMPBOOKS_DATABASE_ROLE` empty), or a migration URL is set and the runtime login is a superuser
- **THEN** the server refuses to start unless `CAMPBOOKS_DATABASE_ALLOW_RLS_BYPASS` is true

#### Scenario: A login holding the definer role
- **WHEN** the role the server queries as is a member of `campbooks_definer`
- **THEN** the server refuses to start naming `campbooks_definer`, with or without a separate migration login

#### Scenario: An old PostgreSQL
- **WHEN** the server connects to PostgreSQL 14
- **THEN** it refuses to start, naming PostgreSQL 14 and the minimum, 15
