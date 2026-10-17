# access Specification

## Purpose
Who owns each resource and who else may do what with it, declared once per
module and checked in one place (ADR 0017): the resource catalogue, the
central check every handler asks, the per-operation access declarations,
and the structural guards (`cargo xtask arch`, the access matrix tests) that
keep new code inside the model. Row-level security stays underneath as the
second layer.

## Requirements

### Requirement: Every resource type is catalogued
Every module SHALL declare each resource type it serves once, in its
descriptor. The declaration SHALL give the type's tenancy (workspace,
account, instance or public), its owner (the member who created or
connected it, the workspace's owners, the account, or nobody), the actions
its owner takes, and the actions granted to anyone else: a minimum
workspace role, a minimum role in the resource's container, the owner while
they hold a container role, client tokens of the workspace (an explicit
grant), instance administrators, or anyone. A client a member registered
SHALL act for that member: it gets their access and no more, and gets 401
once they have left the workspace or the client is removed. Inbox and
inbox-contained types SHALL grant nothing to client tokens as such. The server SHALL refuse to
start with a type declared twice, a type not named after its module, or a
grant naming a container role its container does not have.

#### Scenario: A duplicate type
- **WHEN** two modules declare `conversations.inbox`
- **THEN** the registry fails to build and names the type

### Requirement: One central check, deny by default
Every handler that acts in a workspace or for an account SHALL ask the
central check before it reads or changes a resource. The check SHALL
answer from the catalogue and the owning module's facts about the instance
(whether it exists in the workspace, its owner, its container and the
caller's role there, and whether it is kept read-only). A caller who may
not read a resource SHALL get not found (404) whatever the action. A caller
who may read it but not take the action SHALL get forbidden (403). An
unknown type, or a resource the owning module does not find, SHALL be not
found.

#### Scenario: An admin and a member's inbox
- **WHEN** a workspace admin who is not a member of an inbox asks to read one of its conversations
- **THEN** the answer is 404, the same as for a conversation that does not exist

#### Scenario: A reader writes
- **WHEN** a reader of an inbox archives one of its conversations
- **THEN** the answer is 403 `inbox_read_only`

### Requirement: Operations declare what they do to which resource
Every operation SHALL declare in `x-campbooks.access` the catalogued
resource type it acts on and the action. `cargo xtask arch` SHALL fail on:
- an undeclared operation;
- a declaration naming an unknown type, or an action no one is granted;
- a workspace or account operation whose handler never calls the check
  with its declared type (by name, by a constant holding it, or through a
  helper listed for that type), unless it is listed as pending with its
  tracking issue.

#### Scenario: A new route without a declaration
- **WHEN** a module adds an operation with no `access` in its `x-campbooks`
- **THEN** `cargo xtask arch` fails naming the operation

### Requirement: The access matrix
A test generated from the catalogue SHALL seed an instance of every
workspace resource type in one workspace. It SHALL check that the check's
answer for each kind of caller (owner, admin, member, inbox reader, sender
and connector, client token) and each action is what the catalogue
declares for that caller's real relation to the instance, and that a member
of another workspace gets not found. A second test, generated from the
OpenAPI document, SHALL call every bearer operation:
- as an anonymous caller: 401;
- as a person in no workspace, a member who left, a member of another
  workspace, an instance administrator and another workspace's client
  token: the same 404, with the same body and headers, as for a workspace
  that does not exist;
- with a client registered by a member who left: 401;
- as each kind of member, a client token and a client an admin registered:
  404, 403 or an answer past the access checks, as the catalogue declares;
- the same again once the mailbox is disconnected with its mail kept
  (read-only) and with its mail deleted (404 for every one of its records).

Lists answered to a caller who sees no inbox SHALL carry none of its ids
and no counts of it. A resource type or an operation the matrix cannot
seed or call SHALL fail it until it says how.

#### Scenario: A new resource type
- **WHEN** a module declares a workspace resource type the matrix does not seed
- **THEN** the matrix test fails naming it

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

### Requirement: Events belong to their transaction's workspace and have a consumer
Publishing an event of one workspace in a transaction set to another, or in
a transaction with no workspace set, SHALL fail, and row-level security on
the event log SHALL refuse the former too. Every
published event SHALL have a consumer, or an entry in `architecture.toml`
saying why it has none.

#### Scenario: A cross-workspace publish
- **WHEN** a handler working in workspace A publishes an event of workspace B
- **THEN** the publish fails and nothing is written

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
