## MODIFIED Requirements

### Requirement: Every kernel table has row-level security or a stated reason
Every table of schemas `kernel` and `bff` SHALL enable and force row-level
security and carry at least one policy, none of them a permissive policy for
the runtime role (or everyone) admitting every row (expressions, if any,
that read no column, call no function and hold no subquery or parameter:
`USING (true)`, `USING (1 = 1)`, none), or SHALL grant the runtime role no
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
- **WHEN** a migration adds a view or a materialized view to schema `kernel` that the runtime role may read, or a permissive policy `USING (true)` or `USING (1 = 1)` for everyone or the runtime role on a kernel table
- **THEN** the server's test fails naming it, while a policy for another role, a restrictive one, or a view the runtime role may not read passes
