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
A workspace transaction SHALL NOT read, update, delete or lock the
instance-wide rows (no workspace) of the event log and the job queue, nor
turn one of its own rows into an instance-wide one. It SHALL add an
instance-wide row only as a fresh event whose name, or a fresh delivery of
such an event to a consumer whose kind, is listed in
`kernel.instance_rows_from_workspaces` (pending, unclaimed, due now, keyed
by its event, no ordering key), and SHALL NOT enqueue any other
instance-wide job. The kernel SHALL publish such an event under an id it
chooses, reading nothing back. The list SHALL hold exactly the events declared
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

#### Scenario: Reading an account's events from a workspace
- **WHEN** a transaction working in workspace A selects the event log's `identity.signed_out_everywhere` and `identity.account_suspended` rows, any row naming an account, or the queue's instance-wide deliveries
- **THEN** it sees none of them, while the kernel scope sees them all

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
- **THEN** the event and its deliveries are added under the id the publish returns, and the worker runs them

#### Scenario: Refused by the kernel
- **WHEN** a transaction working in workspace A enqueues an instance-wide job, or publishes an instance-wide event not declared `PUBLISHED_IN_WORKSPACES`
- **THEN** the kernel refuses it with an invalid-argument error and nothing is added

#### Scenario: The list matches the registry
- **WHEN** the server's registry is built and the migrated list is read
- **THEN** they name the same events and delivery kinds, and the runtime role may read the list but not change it

#### Scenario: Instance-wide jobs keep running
- **WHEN** an instance-wide job fails once, another's worker dies holding it, and a third fails for good, as the runtime role
- **THEN** the first is retried and succeeds, the second is reaped and run again, and the third is recorded as failed

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
key and SHALL NOT add or change one, nor delete one other than through the
workspace purge's function. Request bodies SHALL never be stored, only a
hash of the request, and no stored answer SHALL hold a secret (see
`operation-catalogue`). A person's keys through one app SHALL be that
app's alone: the web app and every app acting for the same person keep
separate keys.

A key SHALL record the workspace of the route it was first used on: the
`{workspace_id}` segment of the matched route, percent-decoded as the
router decodes it for the handler, never a value from the request's body
or query, and none for a route outside a workspace. A key taken again once
expired SHALL record its new route's workspace. A
workspace's purge SHALL delete its keys, expired or not, only through
`kernel.purge_workspace_idempotency(workspace, batch)`. That function SHALL
be a `SECURITY DEFINER` function owned by `campbooks_definer`, with a pinned
`search_path`. It SHALL refuse a transaction that does not work in that
workspace, and SHALL answer only how many keys it deleted.

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

#### Scenario: A key's workspace comes from the route
- **WHEN** keyed requests go to a route of workspace A and to one of workspace B, both with a body naming workspace B, and to a route outside any workspace
- **THEN** the keys record A, B and none

#### Scenario: A percent-encoded workspace segment
- **WHEN** a keyed request goes to a workspace route whose `{workspace_id}` segment is percent-encoded
- **THEN** the key records the decoded workspace

#### Scenario: An expired key taken again
- **WHEN** a key used on workspace A's route has expired and is used on workspace B's
- **THEN** the request runs again and the key records B

#### Scenario: A workspace's purge
- **WHEN** the purge function runs for workspace A in a transaction that set nothing, in one working in workspace B, and in one working in A
- **THEN** the first two are refused and delete nothing, and the third deletes A's keys only while still seeing no key

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
security and carry at least one policy, none of them a permissive policy for
the runtime role (or everyone) admitting every row, to read or to write:
no expression at all, or a `USING` or `WITH CHECK` expression that does
not depend on the row (`USING (true)`, `USING (1 = 1)`) and is not a lone
`false`. A policy reading every row whose writes are checked counts as
open. Otherwise the table SHALL grant the runtime role no
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
- **WHEN** a migration adds a view or a materialized view to schema `kernel` that the runtime role may read, or a permissive policy `USING (true)`, `USING (1 = 1)` or `USING (true) WITH CHECK (<a column test>)` for everyone or the runtime role on a kernel table
- **THEN** the server's test fails naming it, while a policy for another role, a restrictive one, `USING (false)`, or a view the runtime role may not read passes

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
missing or empty sealed access or refresh token, an empty ID token or a
missing deadline; a missing ID token keeps the stored one.

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

### Requirement: A workspace's event alone is no proof
Any transaction working in a workspace can append that workspace's events,
so a consumer whose action reaches past the workspace (an account's ways in
and sessions, an app's refresh tokens, a grant at its provider) SHALL check
the state the event claims, through the owning module's port, before
acting, and SHALL do nothing while that state says otherwise. A port that
cannot answer SHALL make the delivery retry, never act on a guess.
`identity.forget_connected_way_in` (`connectors.connection_removed`) SHALL
act only when the connectors module no longer holds that connection for the
record's account; `identity.name_connected_mailbox`
(`connectors.connection_added`) SHALL name a record only with the
connection the connectors module holds for that grantor and provider
account; `identity.revoke_departed_consents` and
`connectors.revoke_departed_grants` (`workspaces.member_left`) SHALL act only
when the person is no longer a member of the workspace;
`identity.remove_workspace_clients` and `connectors.revoke_workspace_grants`
(`workspaces.workspace_deleting`) SHALL act only when the workspace is no
longer live. A consumer that acts at a provider on a member's behalf counts
as reaching past the workspace: `connectors.queue_send`,
`connectors.send_message` and `connectors.request_changes` SHALL send or
change mail only as the conversations module's outbound-requests port
confirms it. Consumers whose effects stay within the event's own workspace
(its own data, which the same transaction could change directly) SHALL NOT
be required to check.

#### Scenario: A forged removal of a live connection
- **WHEN** a transaction working in workspace A appends `connectors.connection_removed` for a connection the connectors module still holds, the only mailbox of a way in a connect created
- **THEN** the way in, its mailbox record and its sessions stay, and no `identity.way_in_removed` is published

#### Scenario: A forged connection names nothing
- **WHEN** `connectors.connection_added` names a connection the connectors module does not hold for its grantor
- **THEN** no mailbox record is named with it, and the connection it does hold still names the record

#### Scenario: A forged departure
- **WHEN** `workspaces.member_left` names someone who is still a member of that workspace
- **THEN** the apps they allowed there keep their consents and refresh tokens, and their connections there stay, unrevoked at the provider
- **AND** once they have left, the same event revokes them

#### Scenario: A forged deletion
- **WHEN** `workspaces.workspace_deleting` names a workspace that is still live
- **THEN** its clients, the consents bound to it and its connections stay, unrevoked at the provider
- **AND** once it is deleted, the same event removes and revokes them

#### Scenario: A forged send or change request
- **WHEN** a transaction working in workspace A appends `conversations.send_requested` or `conversations.changes_requested` that the conversations module's records do not bear out
- **THEN** its mailbox sends nothing and no change is made at the provider
