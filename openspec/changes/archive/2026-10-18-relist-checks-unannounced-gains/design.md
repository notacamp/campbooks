## Context

A stream lists the containers its caller may see once and keeps the list
until a change that may change access is numbered in a range it read, or a
heartbeat comes. Only the heartbeat's listing used to check for a container
gained with no announcing change.

## Decisions

- **What counts as announced.** The container changes in the range that
  made the stream list again are read back (record and container of each,
  a handful of rows at most). A gained container one of them names was
  announced: its changes are read from the cursor under the new list, so
  nothing was read past. Any other gain was not, and resets.
- **Every listing checks.** A heartbeat names nothing, nor does a change of
  access (membership or role), so any gain there resets. Today a role never
  changes which inboxes a member sees, so that path never gains one.
- **A harmless extra reset.** A listing that lands between a grant's commit
  and the sequencer numbering its announcement sees the new container with
  no change naming it yet, so it can send one extra  per container:
  the client refetches once, and nothing is lost or shown twice.
