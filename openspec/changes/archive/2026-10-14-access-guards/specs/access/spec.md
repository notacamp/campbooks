## ADDED Requirements

### Requirement: Every resource type is catalogued
Every module SHALL declare each resource type it serves once, in its
descriptor. The declaration SHALL give the type's tenancy (workspace,
account, instance or public), its owner (the member who created or
connected it, the workspace's owners, the account, or nobody), the actions
its owner takes, and the actions granted to anyone else: a minimum
workspace role, a minimum role in the resource's container, the owner while
they hold a container role, client tokens of the workspace (an explicit
grant), instance administrators, or anyone. The server SHALL refuse to
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
- a workspace or account operation whose handler never calls the check,
  unless it is listed as pending with its tracking issue.

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
- as a person in no workspace, a member of another workspace, an instance
  administrator and another workspace's client token: the same 404, byte
  for byte, as for a workspace that does not exist;
- as each kind of member and a client token: 404, 403 or an answer past the
  access checks, as the catalogue declares.

Lists answered to a member who sees no inbox SHALL carry none of its ids
and no counts of it. A resource type or an operation the matrix cannot
seed or call SHALL fail it until it says how.

#### Scenario: A new resource type
- **WHEN** a module declares a workspace resource type the matrix does not seed
- **THEN** the matrix test fails naming it

### Requirement: Module code stays inside a workspace transaction
Module code SHALL read and write workspace tables only in a transaction
that sets the workspace (`begin_for`, a job's `tx`). Opening a transaction
without one, or querying a workspace table on the pool, SHALL fail
`cargo xtask arch`, except in files listed in `architecture.toml` with the
reason.

#### Scenario: A query on the pool
- **WHEN** a module runs `SELECT … FROM conversations.messages` on `state.db` directly
- **THEN** `cargo xtask arch` fails naming the file and the table

### Requirement: Events belong to their transaction's workspace and have a consumer
Publishing an event of one workspace in a transaction set to another SHALL
fail, and row-level security on the event log SHALL refuse it too. Every
published event SHALL have a consumer, or an entry in `architecture.toml`
saying why it has none.

#### Scenario: A cross-workspace publish
- **WHEN** a handler working in workspace A publishes an event of workspace B
- **THEN** the publish fails and nothing is written
