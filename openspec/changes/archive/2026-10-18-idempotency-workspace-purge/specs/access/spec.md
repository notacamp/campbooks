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
key and SHALL NOT add or change one, nor delete one other than through the
workspace purge's function. Request bodies SHALL never be stored, only a
hash of the request, and no stored answer SHALL hold a secret (see
`operation-catalogue`). A person's keys through one app SHALL be that
app's alone: the web app and every app acting for the same person keep
separate keys.

A key SHALL record the workspace of the route it was first used on: the
`{workspace_id}` segment of the matched route, never a value from the
request's body or query, and none for a route outside a workspace. A
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

#### Scenario: A workspace's purge
- **WHEN** the purge function runs for workspace A in a transaction that set nothing, in one working in workspace B, and in one working in A
- **THEN** the first two are refused and delete nothing, and the third deletes A's keys only while still seeing no key
