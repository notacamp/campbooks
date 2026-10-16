## ADDED Requirements

### Requirement: An unset transaction sees nothing
A transaction that sets neither a workspace nor the kernel scope SHALL see
no row of any table with row-level security, the kernel's event log, job
queue, change feed and blob catalogue included. Work across workspaces
SHALL need the kernel scope (`campbooks.kernel_scope = 'on'`), which only
`kernel::db::begin_kernel` and a job's `kernel_tx` set; only the kernel
SHALL name the setting or its constant (nor, outside it, the admin reads'
constant), and outside the kernel only the files listed under
`[[access.kernel_scope]]` in `architecture.toml`, each with its reason,
SHALL use either function by any name or form (`cargo xtask arch`). A workspace transaction SHALL keep
seeing its own rows and the instance-wide rows of the event log and the
queue. Module maintenance policies SHALL admit the kernel scope, never a
transaction that merely set nothing. An instance-wide event or job SHALL
still be publishable or enqueueable from a transaction that set nothing,
the kernel lifting that transaction into its scope for its own insert only.

#### Scenario: Every table, nothing set
- **WHEN** rows exist in a sample of tables of every kind (the kernel's, modules', the workspaces themselves) and the runtime role opens a transaction that sets nothing
- **THEN** every table with row-level security answers zero rows, a table added later included

#### Scenario: The kernel's machinery
- **WHEN** jobs of two workspaces and an instance-wide job are queued, as the runtime role
- **THEN** the worker claims them fairly and completes them, delivers events of a workspace and instance-wide events, runs the schedules, prunes, sweeps blobs, replays and serves the admin's queue under the kernel scope

#### Scenario: A module opening the kernel scope
- **WHEN** a module file not listed under `[[access.kernel_scope]]` calls `begin_kernel` or `kernel_tx`, renames one (`use … as`), calls it as `JobContext::kernel_tx(ctx)` or takes it as a function value, or code outside the kernel names `campbooks.kernel_scope`, `KERNEL_SCOPE_SETTING` or `ADMIN_READS_SETTING`
- **THEN** `cargo xtask arch` fails naming the file

### Requirement: Kernel writes refuse a transaction that cannot reach their workspace
Enqueueing a workspace's job, recording a change of the feed, attaching or
detaching a blob and the kernel's per-workspace purges SHALL fail
(`InvalidArgument`) in a transaction that set no workspace and is not in
the kernel scope, or that works in another workspace, writing nothing,
rather than silently reaching no row.

#### Scenario: Enqueue without a workspace set
- **WHEN** code enqueues a job for workspace A in a transaction that set no workspace
- **THEN** the enqueue fails and no job is queued

#### Scenario: Attach in another workspace
- **WHEN** code attaches workspace A's blob in a transaction working in workspace B
- **THEN** the attach fails and no reference is recorded

## MODIFIED Requirements

### Requirement: Module code stays inside a workspace transaction
Module code SHALL read and write workspace tables only in a transaction
that sets the workspace (`begin_for`, a job's `tx`), or in the kernel scope
where `architecture.toml` allows it. Opening a transaction or a connection
without one (`.begin()`, `.acquire()`, `Acquire::begin`), or querying a
workspace table on the pool, including through SQL kept in a `const` of the
same file, SHALL fail `cargo xtask arch`, in the modules, the server and the
kernel, except in files listed in `architecture.toml` with the reason.

#### Scenario: A query on the pool
- **WHEN** a module runs `SELECT … FROM conversations.messages` on `state.db` directly
- **THEN** `cargo xtask arch` fails naming the file and the table

#### Scenario: A connection taken from the pool
- **WHEN** server or module code calls `state.db.acquire()` outside the listed files
- **THEN** `cargo xtask arch` fails naming the file

#### Scenario: SQL in a constant
- **WHEN** a module keeps `const SQL: &str = "SELECT … FROM conversations.messages"` and runs `sqlx::query(SQL)` on the pool
- **THEN** `cargo xtask arch` fails naming the file and the table
