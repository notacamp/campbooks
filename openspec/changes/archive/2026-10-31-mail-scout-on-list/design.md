## Decisions

- Same shape as `tables`: one boolean on the list answer, read through the
  list query that is already cached, so the answer costs no request.
- `useMailScoutAvailable(workspaceId)` keeps its name and seam; the `probe`
  page argument goes, as nothing is probed.
