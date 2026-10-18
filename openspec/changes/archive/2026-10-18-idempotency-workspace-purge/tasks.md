## 1. Keys name their workspace

- [x] 1.1 Migration: nullable `kernel.idempotency.workspace_id` and a partial index
- [x] 1.2 The claim records the matched route's `{workspace_id}` segment (parsed as a workspace id), never the body or the query

## 2. The purge

- [x] 2.1 `kernel.purge_workspace_idempotency(workspace, batch)`: a definer owned by `campbooks_definer`, refusing a transaction not working in that workspace; policies admitting only that role to the workspace's keys
- [x] 2.2 `idempotency::purge_workspace` loops over batches in the caller's transaction
- [x] 2.3 `workspaces.purge_workspace` calls it

## 3. Tests

- [x] 3.1 A workspace route's key records it despite a `workspace_id` in the body; a route outside a workspace records none
- [x] 3.2 The function refuses an unset transaction and another workspace's; in the workspace it deletes only that workspace's keys, and the transaction still sees none
- [x] 3.3 The workspace purge deletes its unexpired keys and leaves another workspace's and instance-wide ones
- [x] 3.4 The definer role's grants list includes `kernel.idempotency` (DELETE, SELECT)
- [x] 3.5 A percent-encoded workspace segment records the decoded workspace; an expired key taken again records its new workspace
