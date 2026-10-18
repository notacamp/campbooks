## Why

An idempotency key's stored answer can hold what the operation returned
(a draft's text, a contact's name) for the key's day. Keys had no
workspace, so purging a workspace could not reach them
(campbooks-cloud#65, PR 2). They expire within a day, so for a purge 30 days
after a deletion they are long gone. A purge run at once does find them
unexpired: after a database restore, with the operator's `purge-again`
(campbooks-cloud#65, PR 1), the keys come back with the backup.

## What Changes

- **A key names its workspace.** `kernel.idempotency.workspace_id` is set
  when a key is taken, from the `{workspace_id}` segment of the matched
  route alone (the same segment a replay's access check reads), never from
  the request's body or query. Routes outside a workspace record none.
- **The purge deletes them.** `workspaces.purge_workspace` deletes the
  workspace's keys, expired or not, in its own transaction, through
  `kernel.purge_workspace_idempotency(workspace, batch)`. This is a
  `SECURITY DEFINER` function owned by `campbooks_definer`, like the
  rate-limit and rule-resign functions. It refuses any transaction that
  does not work in that workspace, and answers a count.
- **Keys stay their caller's.** No workspace transaction reads a key or
  writes one directly. Two policies admit `campbooks_definer`, and no other
  role, to find and delete the keys of the transaction's workspace. The
  runtime role's privileges are unchanged.

## Impact

- Spec: access (Idempotency keys belong to their caller).
- Code:
  - kernel `idempotency` (the claim records the workspace;
    `purge_workspace`);
  - the workspaces module's `PurgeWorkspace`;
  - the server's definer-grant test.
- Migration `20261022050100_kernel__idempotency_workspace`. It is
  catalog-only: a nullable column, a partial index on a table of a day's
  keys, two policies and the function.
- Account erasure (deleting a caller's keys by principal) comes with
  campbooks-cloud#65 PR 4.
