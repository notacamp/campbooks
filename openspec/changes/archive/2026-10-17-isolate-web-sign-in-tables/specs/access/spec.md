## ADDED Requirements

### Requirement: The web sign-in's tables are reached only through the BFF's functions
The runtime role SHALL hold no privilege on `bff.sessions`,
`bff.login_transactions` and `bff.retired_tokens`, which SHALL enable
row-level security with no policy. It SHALL reach them only through the
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
values, a landing that is not a same-origin path, a sign-in longer than an
hour and deadlines out of order.

#### Scenario: Direct access as the runtime role
- **WHEN** a transaction of the runtime role selects, updates, deletes, inserts, truncates or locks any of the three tables while a session, a sign-in and a retired token exist
- **THEN** each statement is refused (permission denied), and the session still works

#### Scenario: Another account's session
- **WHEN** two people are signed in, and the runtime role calls the functions with the first person's token hash, an unknown hash, an empty or `NULL` hash, and ends the first person's account sessions
- **THEN** it finds only the first person's session, nothing for the others, and only the first person is signed out

#### Scenario: Everything still works through the functions
- **WHEN** a person signs in, the cookie rotates and is reissued, the access token is refreshed, the person signs out, signs out everywhere or is suspended, and the prune runs, all as the runtime role
- **THEN** each behaves as before, and the prune deletes ended sessions and expired sign-ins only

## MODIFIED Requirements

### Requirement: An unset transaction sees nothing
A transaction that sets neither a workspace nor a scope SHALL see no row of
any table with row-level security, the kernel's event log, job queue,
change feed and blob catalogue included. Work across workspaces on the
kernel's tables SHALL need the kernel scope (`campbooks.kernel_scope =
'on'`), which only `kernel::db::begin_kernel` and a job's `kernel_tx` set.
A module scheduler's dispatcher SHALL work in the narrower maintenance
scope (`campbooks.maintenance_scope = 'on'`), which only
`kernel::db::begin_maintenance` and a job's `maintenance_tx` set, which only
its own table's maintenance policies admit and which no kernel table admits;
it SHALL enqueue each run with `jobs::enqueue_in` / `enqueue_to_in`, which
set the run's workspace for that enqueue only and clear it again in the
same transaction. Only the kernel SHALL name a scope's setting
(`campbooks.kernel_scope`, `campbooks.maintenance_scope`,
`campbooks.admin_reads`) or the idempotency layer's caller
(`campbooks.idempotency_caller`), its constant or its last word on its own
(`kernel_scope`, `maintenance_scope`, `admin_reads`, `idempotency_caller`,
in code or a string,
any case, tests included, also when spelled with escapes such as `\x5f` or
`\u{5f}`), and outside the kernel only the files listed for that scope in
`architecture.toml` (`[[access.kernel_scope]]`,
`[[access.maintenance_scope]]`), each with its reason, SHALL use its
openers by any name or form (`cargo xtask arch`); an entry for one scope
SHALL NOT open the other, and the admin reads and the idempotency caller
have no list. A file's test
module SHALL end the check only when it is the file's last item, at its top
level. A workspace transaction SHALL keep seeing its own rows and the
instance-wide rows of the event log and the queue. Module maintenance
policies SHALL admit the maintenance scope, never the kernel scope nor a
transaction that merely set nothing; the kernel scope SHALL appear only in
the kernel tables' policies (`workspace_isolation`; `kernel_scope_only` on
the schedules' last ticks and the rate-limit counters; the expired-only
read and prune of the idempotency keys), `USING` and `WITH CHECK` read
alike, and the idempotency caller only in the idempotency keys'
`caller_isolation`. An instance-wide event or job SHALL still be publishable or
enqueueable from a transaction that set nothing, the kernel lifting that
transaction into its scope for its own insert only.

#### Scenario: Every table, nothing set
- **WHEN** rows exist in a sample of tables of every kind (the kernel's, modules', the workspaces themselves) and the runtime role opens a transaction that sets nothing
- **THEN** every table with row-level security answers zero rows, a table added later included, and those the runtime role may not read at all (`kernel.rate_limits`, `bff.sessions`, `bff.login_transactions`, `bff.retired_tokens`) refuse the read

#### Scenario: The kernel's machinery
- **WHEN** jobs of two workspaces and an instance-wide job are queued, as the runtime role
- **THEN** the worker claims them fairly and completes them, delivers events of a workspace and instance-wide events, runs the schedules, prunes, sweeps blobs, replays and serves the admin's queue under the kernel scope

#### Scenario: The maintenance scope
- **WHEN** a transaction of the runtime role opens the maintenance scope while the event log, the queue, the change feed and the blob catalogue hold rows
- **THEN** it sees none of them, enqueueing a workspace's job is refused until it sets that workspace, and a workspace's job asking for `maintenance_tx` fails

#### Scenario: Enqueueing in a workspace for one enqueue
- **WHEN** a maintenance transaction enqueues jobs of two workspaces with `enqueue_in`, and a transaction that works in a workspace calls `enqueue_in`
- **THEN** each job is enqueued in its workspace and the transaction works in none afterwards, and the second call is refused

#### Scenario: Every policy is read
- **WHEN** the database's policies are listed, every schema's and both `USING` and `WITH CHECK`
- **THEN** none admits a transaction for setting no workspace, the kernel scope appears only in the kernel's `workspace_isolation` policies, `kernel_scope_only` on `kernel.schedules` and `kernel.rate_limits` and the two expired-only policies of `kernel.idempotency`, the idempotency caller only in `kernel.idempotency`'s `caller_isolation`, and the maintenance scope only in the four maintenance policies

#### Scenario: A module opening the kernel scope
- **WHEN** a module file not listed for that scope calls `begin_kernel`, `kernel_tx`, `begin_maintenance` or `maintenance_tx`, renames one (`use … as`), calls it as `JobContext::kernel_tx(ctx)` or takes it as a function value, or code outside the kernel names `campbooks.kernel_scope`, `campbooks.maintenance_scope`, `campbooks.admin_reads`, `campbooks.idempotency_caller`, `KERNEL_SCOPE_SETTING`, `MAINTENANCE_SCOPE_SETTING`, `ADMIN_READS_SETTING` or `IDEMPOTENCY_CALLER_SETTING`
- **THEN** `cargo xtask arch` fails naming the file

#### Scenario: A setting name built at runtime
- **WHEN** code outside the kernel and the listed files writes `set_config('campbooks.' || 'kernel_scope', …)`, `SET LOCAL campbooks."kernel_scope"`, `concat!("campbooks.", stringify!(admin_reads))`, `format!("campbooks.{}", "maintenance_scope")`, `"campbooks.kernel\x5fscope"` or `"kernel\u{5f}scope"`, a test included
- **THEN** `cargo xtask arch` fails naming the file

#### Scenario: Text that hides a call
- **WHEN** a module file opens the kernel scope after a C raw string holding `//` (`cr#"x"//"#; begin_kernel(pool);`), after a string or comment holding `#[cfg(test)]` and `mod tests {` on the next line, or after a `#[cfg(test)] mod tests {}` nested in another module, inside a `macro_rules!` body, or followed by more code
- **THEN** `cargo xtask arch` still fails naming the file

### Requirement: Rate-limit counters are reached only through the kernel's functions
The runtime role SHALL hold no privilege on `kernel.rate_limits`, which
SHALL enable and force row-level security (kernel scope only). It SHALL
count a hit only through `kernel.take_rate_limit(bucket, window_seconds,
cost)` and prune ended windows only through
`kernel.prune_rate_limits(batch)`: `SECURITY DEFINER` functions with a pinned
`search_path` that turn the kernel scope on for their own statement and
restore the caller's setting. The prune SHALL delete only windows that have
ended, checked again on each row it deletes, so a window a hit renews while
the batch is chosen is kept. `take_rate_limit` SHALL refuse a bucket that
is not 32 bytes, a window outside (0, 86400] seconds and a cost below 1. No
transaction of the runtime role, in a workspace or not, SHALL read, reset,
lower or write a counter otherwise, and every limit SHALL keep limiting.

#### Scenario: Reading or resetting a counter
- **WHEN** a transaction of the runtime role, working in a workspace or setting nothing, selects, updates, deletes or inserts rate-limit counters
- **THEN** each statement is refused (permission denied), and a key over its limit is still refused

#### Scenario: Bad input to the function
- **WHEN** the runtime role calls `kernel.take_rate_limit` with an 8-byte bucket, a window of 0 or of 86 401 seconds, or a cost of 0 or below
- **THEN** each call fails and no counter is written

#### Scenario: Limits and pruning as the runtime role
- **WHEN** the runtime role hits a limit of two per minute three times, then the window ends and the prune runs
- **THEN** the third hit is refused, the prune deletes the ended windows, and the next hit is allowed

### Requirement: Every kernel table has row-level security or a stated reason
Every table of schemas `kernel` and `bff` SHALL enable and force row-level
security and carry at least one policy, or SHALL grant the runtime role no
privilege of any kind (reached through `SECURITY DEFINER` functions only), or
SHALL be listed, in the server's test, as instance-only with the reason it
needs none; a listed table SHALL grant the runtime role no `INSERT`,
`UPDATE`, `DELETE` or `TRUNCATE`. Today the list SHALL hold only
`kernel.instance_rows_from_workspaces` (read-only to the runtime role,
written by migrations). Every `SECURITY DEFINER` function SHALL set exactly
`search_path = pg_catalog, pg_temp` and SHALL NOT be executable by `PUBLIC`.
A migration MAY pin a function's `search_path` to exactly `pg_catalog,
pg_temp` (`pg_temp` last, no other entry) in its definition and SHALL NOT
change the session's (`cargo xtask arch`).

#### Scenario: A new kernel table without row-level security
- **WHEN** a migration adds a table to schema `kernel` without enabling and forcing row-level security, and it is not listed with a reason
- **THEN** the server's test fails naming the table

#### Scenario: A function's pinned path
- **WHEN** a kernel migration defines a `SECURITY DEFINER` function with `SET search_path = pg_catalog, pg_temp`, and others run `SET search_path = pg_catalog, pg_temp;`, end the file with it, set a function's path to another schema, to `pg_catalog, pg_temp, kernel` or to `pg_temp, pg_catalog`
- **THEN** `cargo xtask arch` accepts the first and fails naming the others

#### Scenario: A BFF table the runtime role may reach
- **WHEN** a migration grants the runtime role any privilege on a table of schema `bff` without forced row-level security and a policy
- **THEN** the server's test fails naming the table

#### Scenario: A definer function open to everyone
- **WHEN** a migration defines a `SECURITY DEFINER` function without the pinned path, or without revoking it from `PUBLIC`
- **THEN** the server's test fails naming the function
