## MODIFIED Requirements

### Requirement: Instance-wide rows change only in the kernel scope
A workspace transaction SHALL read the instance-wide rows (no workspace) of
the event log and the job queue, but SHALL NOT update, delete or lock them,
nor turn one of its own rows into an instance-wide one. It SHALL add an
instance-wide row only as a fresh event whose name, or a fresh delivery of
such an event to a consumer whose kind, is listed in
`kernel.instance_rows_from_workspaces` (pending, unclaimed, due now, keyed
by its event, no ordering key), and SHALL NOT enqueue any other
instance-wide job. The list SHALL hold exactly the events declared
`DomainEvent::PUBLISHED_IN_WORKSPACES` and their consumers; the runtime role
SHALL only read it. Outside the kernel scope no row, a workspace's own
included, SHALL carry a schedule's unique key (`schedule:<name>`). The
kernel SHALL refuse an instance-wide job, or an instance-wide event not
declared `PUBLISHED_IN_WORKSPACES`, from a workspace transaction before
asking the database. A delivery SHALL be handled only when its event is
logged in the event's own workspace (none for an instance-wide type);
otherwise it SHALL fail for good without running its handler. Only the
kernel scope SHALL read or write the schedules' last ticks
(`kernel.schedules`). Only the kernel scope SHALL change instance-wide rows:
claiming, renewing, releasing, retrying, reaping and completing
instance-wide jobs, pruning and the admin's queue SHALL keep working there.

#### Scenario: Changing an instance-wide job from a workspace
- **WHEN** a transaction working in workspace A updates or deletes the queue's or the event log's instance-wide rows, or locks them with `FOR UPDATE`
- **THEN** no row is changed or locked, and the rows are unchanged once it commits

#### Scenario: Moving a row out of the workspace
- **WHEN** a transaction working in workspace A sets the workspace of one of its own jobs to none
- **THEN** the statement fails

#### Scenario: Forging a schedule's row
- **WHEN** a transaction working in workspace A inserts, as raw SQL, a job holding `schedule:kernel.prune_jobs`, instance-wide or of its own workspace
- **THEN** row-level security refuses it, and the schedule's next tick enqueues as usual

#### Scenario: Forging other instance-wide work
- **WHEN** a transaction working in workspace A inserts, as raw SQL, an instance-wide delivery due never, already claimed, ordered behind another key or keyed otherwise than by its event, a job of an unlisted kind, an event of an unlisted name (`identity.account_suspended`), or a listed event dated in the future
- **THEN** row-level security refuses each one

#### Scenario: An account's event forged in a workspace
- **WHEN** a transaction working in workspace A writes, as raw SQL into its own workspace, an `identity.signed_out_everywhere` naming another person's account and a `bff.end_signed_out_sessions` delivery of it, and the worker runs
- **THEN** the delivery fails for good, its handler never runs, and that person stays signed in

#### Scenario: An event naming another workspace
- **WHEN** a workspace's event whose payload names workspace B is logged in workspace A, with a delivery there
- **THEN** the delivery fails for good and its handler never runs

#### Scenario: The schedules' last ticks
- **WHEN** a transaction working in a workspace, or one that set nothing, reads `kernel.schedules`, sets a schedule's `last_tick` far ahead, or inserts a row
- **THEN** it sees no row, changes none, and the insert is refused

#### Scenario: Adding instance-wide work from a workspace
- **WHEN** a transaction working in workspace A publishes an instance-wide event declared `PUBLISHED_IN_WORKSPACES` (a member removes a workspace client; a departed member's consents are revoked; a removed mailbox takes the account's last way in)
- **THEN** the event and its deliveries are added and the worker runs them

#### Scenario: Refused by the kernel
- **WHEN** a transaction working in workspace A enqueues an instance-wide job, or publishes an instance-wide event not declared `PUBLISHED_IN_WORKSPACES`
- **THEN** the kernel refuses it with an invalid-argument error and nothing is added

#### Scenario: The list matches the registry
- **WHEN** the server's registry is built and the migrated list is read
- **THEN** they name the same events and delivery kinds, and the runtime role may read the list but not change it

#### Scenario: Instance-wide jobs keep running
- **WHEN** an instance-wide job fails once, another's worker dies holding it, and a third fails for good, as the runtime role
- **THEN** the first is retried and succeeds, the second is reaped and run again, and the third is recorded as failed

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
`campbooks.admin_reads`), its constant or its last word on its own
(`kernel_scope`, `maintenance_scope`, `admin_reads`, in code or a string,
any case, tests included, also when spelled with escapes such as `\x5f` or
`\u{5f}`), and outside the kernel only the files listed for that scope in
`architecture.toml` (`[[access.kernel_scope]]`,
`[[access.maintenance_scope]]`), each with its reason, SHALL use its
openers by any name or form (`cargo xtask arch`); an entry for one scope
SHALL NOT open the other, and the admin reads have no list. A file's test
module SHALL end the check only when it is the file's last item, at its top
level. A workspace transaction SHALL keep seeing its own rows and the
instance-wide rows of the event log and the queue. Module maintenance
policies SHALL admit the maintenance scope, never the kernel scope nor a
transaction that merely set nothing; the kernel scope SHALL appear only in
the kernel tables' `workspace_isolation` policies, `USING` and `WITH CHECK`
read alike. An instance-wide event or job SHALL still be publishable or
enqueueable from a transaction that set nothing, the kernel lifting that
transaction into its scope for its own insert only.

#### Scenario: Every table, nothing set
- **WHEN** rows exist in a sample of tables of every kind (the kernel's, modules', the workspaces themselves) and the runtime role opens a transaction that sets nothing
- **THEN** every table with row-level security answers zero rows, a table added later included

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
- **THEN** none admits a transaction for setting no workspace, the kernel scope appears only in the kernel's `workspace_isolation` policies, and the maintenance scope only in the four maintenance policies

#### Scenario: A module opening the kernel scope
- **WHEN** a module file not listed for that scope calls `begin_kernel`, `kernel_tx`, `begin_maintenance` or `maintenance_tx`, renames one (`use … as`), calls it as `JobContext::kernel_tx(ctx)` or takes it as a function value, or code outside the kernel names `campbooks.kernel_scope`, `campbooks.maintenance_scope`, `campbooks.admin_reads`, `KERNEL_SCOPE_SETTING`, `MAINTENANCE_SCOPE_SETTING` or `ADMIN_READS_SETTING`
- **THEN** `cargo xtask arch` fails naming the file

#### Scenario: A setting name built at runtime
- **WHEN** code outside the kernel and the listed files writes `set_config('campbooks.' || 'kernel_scope', …)`, `SET LOCAL campbooks."kernel_scope"`, `concat!("campbooks.", stringify!(admin_reads))`, `format!("campbooks.{}", "maintenance_scope")`, `"campbooks.kernel\x5fscope"` or `"kernel\u{5f}scope"`, a test included
- **THEN** `cargo xtask arch` fails naming the file

#### Scenario: Text that hides a call
- **WHEN** a module file opens the kernel scope after a C raw string holding `//` (`cr#"x"//"#; begin_kernel(pool);`), after a string or comment holding `#[cfg(test)]` and `mod tests {` on the next line, or after a `#[cfg(test)] mod tests {}` nested in another module, inside a `macro_rules!` body, or followed by more code
- **THEN** `cargo xtask arch` still fails naming the file

### Requirement: Module code stays inside a workspace transaction
Module code SHALL read and write workspace tables only in a transaction
that sets the workspace (`begin_for`, a job's `tx`), or in a scope where
`architecture.toml` allows it. Opening a transaction or a connection
without one (`.begin()`, `.acquire()`, `Acquire::begin`), or querying a
workspace table on the pool, including through SQL kept in a `const` of the
same file, SHALL fail `cargo xtask arch`, in the modules, the server and the
kernel, except in files listed in `architecture.toml` with the reason. The
server's entries SHALL name files, never a directory, so a new file is
checked; the web BFF's only entry SHALL be its session store (`store.rs`),
which also opens the token refresh's transaction.

#### Scenario: A query on the pool
- **WHEN** a module runs `SELECT … FROM conversations.messages` on `state.db` directly
- **THEN** `cargo xtask arch` fails naming the file and the table

#### Scenario: A connection taken from the pool
- **WHEN** server or module code calls `state.db.acquire()` outside the listed files
- **THEN** `cargo xtask arch` fails naming the file

#### Scenario: SQL in a constant
- **WHEN** a module keeps `const SQL: &str = "SELECT … FROM conversations.messages"` and runs `sqlx::query(SQL)` on the pool
- **THEN** `cargo xtask arch` fails naming the file and the table

#### Scenario: A new file of the BFF
- **WHEN** a new file under `crates/server/src/bff/`, or the BFF's `mod.rs`, calls `self.db.begin()`
- **THEN** `cargo xtask arch` fails naming the file
