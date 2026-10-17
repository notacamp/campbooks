## ADDED Requirements

### Requirement: Definer functions run as a dedicated owner that cannot log in
Every `SECURITY DEFINER` function SHALL be owned by `campbooks_definer`,
never by a superuser or a role that can log in (such as the migration
login). `campbooks_definer` SHALL NOT log in, be a superuser, bypass
row-level security, create databases or roles, or replicate; it SHALL be a
member of no role, no runtime role SHALL be a member of it, and it SHALL own
nothing but those functions. It SHALL hold exactly the privileges their
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

## MODIFIED Requirements

### Requirement: The web sign-in's tables are reached only through the BFF's functions
The runtime role SHALL hold no privilege on `bff.sessions`,
`bff.login_transactions` and `bff.retired_tokens`, which SHALL enable
row-level security with one policy, admitting only the functions' owner
`campbooks_definer`. It SHALL reach them only through the
`SECURITY DEFINER` functions of schema `bff`, one per operation of the web
BFF: `create_session`, `find_session`, `touch_session`,
`mark_session_delivered`, `rotate_session`, `reissue_session`,
`retired_session`, `end_session`, `lock_session_tokens`,
`store_session_tokens`, `end_account_sessions`, `prune_sessions`,
`start_login`, `take_login` and `prune_logins`. Each SHALL run with a pinned
`search_path`, be executable by the runtime role only, and act on the one
session named by the SHA-256 of a token its cookie presented, the one
sign-in named by its state's SHA-256, or the sessions of one account (or of
one identity-provider session); none SHALL answer a session's token hashes
or list sessions. Only the two prunes SHALL reach across accounts, deleting
only rows ended by the database's clock, at most 10 000 per call, each row
checked again as it is deleted. The functions creating rows SHALL refuse
token, state and browser hashes that are not 32 bytes, missing sealed
values, a sign-in longer than an hour, a session longer than 366 days and
deadlines out of order, and a landing the handler would refuse: longer than
2 048 characters, not starting with exactly one `/`, holding anything but
visible ASCII, a backslash, `"`, `'`, `<`, `>` or a backtick, or naming the
BFF's own paths. `touch_session` SHALL refuse a missing time or idle
deadline, or a deadline not after the use, and `store_session_tokens` a
missing or empty sealed access or refresh token or a missing deadline.

#### Scenario: Direct access as the runtime role
- **WHEN** a transaction of the runtime role selects, updates, deletes, inserts, truncates or locks any of the three tables while a session, a sign-in and a retired token exist
- **THEN** each statement is refused (permission denied), and the session still works

#### Scenario: Another account's session
- **WHEN** two people are signed in, and the runtime role calls the functions with the first person's token hash, an unknown hash, an empty or `NULL` hash, and ends the first person's account sessions
- **THEN** it finds only the first person's session, nothing for the others, and only the first person is signed out

#### Scenario: Everything still works through the functions
- **WHEN** a person signs in, the cookie rotates and is reissued, the access token is refreshed, the person signs out, signs out everywhere or is suspended, and the prune runs, all as the runtime role
- **THEN** each behaves as before, and the prune deletes ended sessions and expired sign-ins only

#### Scenario: Input the BFF never sends
- **WHEN** the runtime role starts a sign-in landing on `//evil.example`, `/\evil.example`, a path with a space or a tab, `/"><script>`, `/bff/logout` or a path of 2 049 characters, touches a session with no time, no deadline or a deadline equal to the use, stores tokens with none or an empty one, or creates a session of 367 days or whose idle deadline is its start
- **THEN** each call fails as invalid input and nothing changes, and a sign-in landing on `/inbox?view=all#top` is accepted

### Requirement: Every kernel table has row-level security or a stated reason
Every table of schemas `kernel` and `bff` SHALL enable and force row-level
security and carry at least one policy, or SHALL grant the runtime role no
privilege of any kind on the table or on any of its columns (reached through
`SECURITY DEFINER` functions only), or SHALL be listed, in the server's
test, as instance-only with the reason it needs none; a listed table SHALL
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
