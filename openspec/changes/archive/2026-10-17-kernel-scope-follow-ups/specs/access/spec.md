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
its own table's maintenance policies admit and which no kernel table admits.
Only the kernel SHALL name either setting, its constant (nor, outside it,
the admin reads' constant) or its last word on its own (`kernel_scope`,
`maintenance_scope`, in code or a string, any case, tests included), and
outside the kernel only the files listed for that scope in
`architecture.toml` (`[[access.kernel_scope]]`,
`[[access.maintenance_scope]]`), each with its reason, SHALL use its
openers by any name or form (`cargo xtask arch`); an entry for one scope
SHALL NOT open the other. A workspace transaction SHALL keep seeing its own
rows and the instance-wide rows of the event log and the queue. Module
maintenance policies SHALL admit the maintenance scope, never the kernel
scope nor a transaction that merely set nothing. An instance-wide event or
job SHALL still be publishable or enqueueable from a transaction that set
nothing, the kernel lifting that transaction into its scope for its own
insert only.

#### Scenario: Every table, nothing set
- **WHEN** rows exist in a sample of tables of every kind (the kernel's, modules', the workspaces themselves) and the runtime role opens a transaction that sets nothing
- **THEN** every table with row-level security answers zero rows, a table added later included

#### Scenario: The kernel's machinery
- **WHEN** jobs of two workspaces and an instance-wide job are queued, as the runtime role
- **THEN** the worker claims them fairly and completes them, delivers events of a workspace and instance-wide events, runs the schedules, prunes, sweeps blobs, replays and serves the admin's queue under the kernel scope

#### Scenario: The maintenance scope
- **WHEN** a transaction of the runtime role opens the maintenance scope while the event log, the queue, the change feed and the blob catalogue hold rows
- **THEN** it sees none of them, enqueueing a workspace's job is refused until it sets that workspace, and a workspace's job asking for `maintenance_tx` fails

#### Scenario: A module opening the kernel scope
- **WHEN** a module file not listed for that scope calls `begin_kernel`, `kernel_tx`, `begin_maintenance` or `maintenance_tx`, renames one (`use … as`), calls it as `JobContext::kernel_tx(ctx)` or takes it as a function value, or code outside the kernel names `campbooks.kernel_scope`, `campbooks.maintenance_scope`, `KERNEL_SCOPE_SETTING`, `MAINTENANCE_SCOPE_SETTING` or `ADMIN_READS_SETTING`
- **THEN** `cargo xtask arch` fails naming the file

#### Scenario: A setting name built at runtime
- **WHEN** code outside the kernel and the listed files writes `set_config('campbooks.' || 'kernel_scope', …)`, `SET LOCAL campbooks."kernel_scope"`, `concat!("campbooks.", stringify!(kernel_scope))` or `format!("campbooks.{}", "maintenance_scope")`, a test included
- **THEN** `cargo xtask arch` fails naming the file

#### Scenario: Text that hides a call
- **WHEN** a module file opens the kernel scope after a C raw string holding `//` (`cr#"x"//"#; begin_kernel(pool);`), or after a string or comment holding `#[cfg(test)]` and `mod tests {` on the next line
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
checked.

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
- **WHEN** a new file under `crates/server/src/bff/` calls `self.db.begin()`
- **THEN** `cargo xtask arch` fails naming the file

## ADDED Requirements

### Requirement: Instance-wide rows change only in the kernel scope
A workspace transaction SHALL read the instance-wide rows (no workspace) of
the event log and the job queue and SHALL insert them (an instance-wide
event or follow-up job, a unique-key enqueue whose duplicate is
instance-wide, reading the inserted row back), but SHALL NOT update,
delete or lock them, nor turn one of its own rows into an instance-wide
one. Only the kernel scope SHALL change instance-wide rows: claiming,
renewing, releasing, retrying, reaping and completing instance-wide jobs,
pruning and the admin's queue SHALL keep working there.

#### Scenario: Changing an instance-wide job from a workspace
- **WHEN** a transaction working in workspace A updates or deletes the queue's or the event log's instance-wide rows, or locks them with `FOR UPDATE`
- **THEN** no row is changed or locked, and the rows are unchanged once it commits

#### Scenario: Moving a row out of the workspace
- **WHEN** a transaction working in workspace A sets the workspace of one of its own jobs to none
- **THEN** the statement fails

#### Scenario: Adding instance-wide work from a workspace
- **WHEN** a transaction working in workspace A enqueues an instance-wide job, enqueues one whose unique key an instance-wide job already holds, and publishes an instance-wide event
- **THEN** the first job and the event's deliveries are added, the second enqueue is a duplicate, and the worker runs them all

#### Scenario: Instance-wide jobs keep running
- **WHEN** an instance-wide job fails once, another's worker dies holding it, and a third fails for good, as the runtime role
- **THEN** the first is retried and succeeds, the second is reaped and run again, and the third is recorded as failed
