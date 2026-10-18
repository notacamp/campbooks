## MODIFIED Requirements

### Requirement: Idempotency keys belong to their caller
`kernel.idempotency` SHALL enable and force row-level security. A key SHALL
be reachable only in a transaction naming its caller
(`campbooks.idempotency_caller`: `user:<length of the subject>:<subject>|azp:<client id>`,
a person through one app, or `client:<client id>`),
which only the kernel's idempotency layer opens
(`kernel::db::begin_as_idempotency_caller`), for the claim, the stored
answer and the release. The kernel scope SHALL read and delete expired
keys only (the hourly prune), and SHALL NOT read a live key nor write any.
A transaction working in a workspace, or one that set nothing, SHALL see no
key and SHALL NOT add, change or delete one. Request bodies SHALL never be
stored, only a hash of the request, and no stored answer SHALL hold a
secret (see `operation-catalogue`). A person's keys through one app SHALL be
that app's alone: the web app and every app acting for the same person keep
separate keys.

#### Scenario: A workspace transaction and the keys
- **WHEN** a key is stored for a caller, and a transaction of the runtime role working in a workspace counts, updates, deletes and inserts idempotency keys
- **THEN** it counts zero, changes and deletes none, the insert is refused by row-level security, and the caller's replay still returns the stored answer

#### Scenario: Another caller's keys
- **WHEN** a transaction names another caller, or names none and sets no workspace
- **THEN** it sees none of the first caller's keys

#### Scenario: The prune
- **WHEN** the hourly prune runs as the runtime role while one key has expired and another is live
- **THEN** it deletes the expired key only, and a transaction in the kernel scope sees no live key and rewrites none

#### Scenario: One person through two apps
- **WHEN** a person's web app and an app acting for them each send a request with `Idempotency-Key: k-1`
- **THEN** each runs once and replays only to its own app, and neither gets 422 for the other's request

### Requirement: Every kernel table has row-level security or a stated reason
Every table of schemas `kernel` and `bff` SHALL enable and force row-level
security and carry at least one policy, none of them a permissive policy for
the runtime role (or everyone) admitting every row (`USING (true)`, `WITH
CHECK (true)` or no expression), or SHALL grant the runtime role no
privilege of any kind on the table or on any of its columns (reached through
`SECURITY DEFINER` functions only), or SHALL be listed, in the server's
test, as instance-only with the reason it needs none. A view or materialized
view in those schemas SHALL grant the runtime role nothing, since row-level
security does not apply through it; a listed table SHALL
grant the runtime role no `INSERT`, `UPDATE`, `DELETE` or `TRUNCATE`, on the
table or a column. Today the list SHALL hold only
`kernel.instance_rows_from_workspaces` (read-only to the runtime role,
written by migrations). Every `SECURITY DEFINER` function SHALL set exactly
`search_path = pg_catalog, pg_temp` and SHALL NOT be executable by `PUBLIC`.
A migration MAY pin a function's `search_path` to exactly `pg_catalog,
pg_temp` (`pg_temp` last, no other entry, ending the name) in its
definition and SHALL NOT change the session's, nor name `search_path` in a
string (`set_config`) (`cargo xtask arch`).

#### Scenario: A new kernel table without row-level security
- **WHEN** a migration adds a table to schema `kernel` without enabling and forcing row-level security, and it is not listed with a reason
- **THEN** the server's test fails naming the table

#### Scenario: A function's pinned path
- **WHEN** a kernel migration defines a `SECURITY DEFINER` function with `SET search_path = pg_catalog, pg_temp`, and others run `SET search_path = pg_catalog, pg_temp;`, end the file with it, set a function's path to another schema, to `pg_catalog, pg_temp, kernel`, to `pg_temp, pg_catalog`, to `pg_catalog, pg_temp$x` or to `pg_catalog, pg_tempx`, or call `set_config('search_path', …)` in a function body or a statement
- **THEN** `cargo xtask arch` accepts the first and fails naming the others

#### Scenario: A BFF table the runtime role may reach
- **WHEN** a migration grants the runtime role any privilege on a table of schema `bff`, or on one of its columns, without forced row-level security and a policy
- **THEN** the server's test fails naming the table

#### Scenario: A definer function open to everyone
- **WHEN** a migration defines a `SECURITY DEFINER` function without the pinned path, or without revoking it from `PUBLIC`
- **THEN** the server's test fails naming the function

#### Scenario: An open view or an open policy
- **WHEN** a migration adds a view or a materialized view to schema `kernel` that the runtime role may read, or a permissive policy `USING (true)` for everyone on a kernel table
- **THEN** the server's test fails naming it, while a policy for another role, a restrictive one, or a view the runtime role may not read passes

### Requirement: Definer functions run as a dedicated owner that cannot log in
Every `SECURITY DEFINER` function SHALL be owned by `campbooks_definer`,
never by a superuser or a role that can log in (such as the migration
login). `campbooks_definer` SHALL NOT log in, be a superuser, bypass
row-level security, create databases or roles, or replicate; it SHALL be a
member of no role, no runtime role SHALL be a member of it, and it SHALL own
nothing but those functions. The server SHALL refuse to start when the role
it queries as is a member of it (see `workspaces`), and a migration SHALL
fail when `campbooks_app` is. The migration login SHALL hold it with
`INHERIT` (`USAGE`), so later migrations can replace the functions; a
migration SHALL grant that to itself when it may (PostgreSQL 16 and later),
and otherwise SHALL fail naming the statement an administrator runs once. It SHALL hold exactly the privileges their
statements need: `USAGE` on schemas `kernel` and `bff`; `SELECT`, `INSERT`,
`UPDATE` and `DELETE` on `kernel.rate_limits` and `bff.sessions`; `SELECT`,
`INSERT` and `DELETE` on `bff.login_transactions`; `SELECT` and `INSERT` on
`bff.retired_tokens`; and no grant on a column, function or database. A
migration SHALL create the role when its login may (a superuser, or a login
with `CREATEROLE`), and otherwise SHALL fail naming the statements an
administrator runs once; it SHALL never leave a function with the login.
Handing the functions over SHALL keep each one executable by the runtime role
only.

#### Scenario: Who owns the definer functions
- **WHEN** every migration has run and the database's `SECURITY DEFINER` functions are listed
- **THEN** each belongs to `campbooks_definer`, which cannot log in, has no privileged attribute, is a member of no role, owns nothing else and holds exactly the privileges above

#### Scenario: A new definer function left with the migration login
- **WHEN** a later migration creates a `SECURITY DEFINER` function without `ALTER FUNCTION … OWNER TO campbooks_definer`
- **THEN** the server's test fails naming the function

#### Scenario: The functions still work for the runtime role
- **WHEN** the runtime role counts rate-limit hits, starts and finishes a sign-in, uses, rotates, refreshes and ends a session, and the prunes run
- **THEN** each behaves as before, and the runtime role still cannot read or write the four tables directly

#### Scenario: A migration login that cannot create roles
- **WHEN** the migration login is neither a superuser nor allowed to create roles and `campbooks_definer` does not exist or cannot be taken
- **THEN** the migration fails, naming `CREATE ROLE campbooks_definer NOLOGIN NOINHERIT` and `GRANT campbooks_definer TO <login>`, and applies once an administrator has run them

#### Scenario: The runtime role holds the owner role
- **WHEN** an administrator grants `campbooks_definer` to `campbooks_app` and the migrations run, or to the login the server queries as and the server starts
- **THEN** the migration fails naming `REVOKE campbooks_definer FROM campbooks_app`, and the server refuses to start naming `campbooks_definer`
