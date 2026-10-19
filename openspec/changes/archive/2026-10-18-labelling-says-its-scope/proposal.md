# Proposal

## Why

From the review of #555 (campbooks-cloud#219): the bounded labelling export
resolves a member by id or address, but the operator only learnt whom it
resolved to, and which inboxes it read, from the result. Saying it first, and
keeping it in the folder, lets them stop a wrong export early and tell later
what a folder holds.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/219

## What Changes

- `campbooks-server labelling export` prints, before it reads any mail, the
  resolved member id, their role and the ids of the inboxes it exports.
- The folder's `.campbooks-labelling` marker records the same: workspace,
  member, role, inboxes and set, one `key value` per line after its header.
- `conversations::labelling::export_inboxes` answers which inboxes an export
  reads (the same rule and refusals as the export itself).

## Non-goals

- No change to what is exported or to `purge` (it still reads the header line).
