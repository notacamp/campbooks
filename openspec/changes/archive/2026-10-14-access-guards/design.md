# Design: access guards

The model and its trade-offs are ADR 0017. This file covers how each piece
is built, what data each module owns, and the events.

## The catalogue and the check (kernel)

- `kernel::authz::{ResourceType, Owner, Grant, Grantee, Action, Tenancy}`:
  declarations. `Resources { types, facts }` sits in `Module.resources`.
  `Registry::build` collects every module's into one `authz::Catalogue` and
  checks names are `<module>.<record>` of the declaring module, unique, and
  that container roles exist. `AppState::with_registry` installs it, and so
  does the server's `with_ports`.
- `ResourceFacts`: one port per module. It answers, in the caller's
  workspace transaction (RLS on), with owner, container, the caller's
  container role and `frozen`. It answers `None` when the resource does not
  exist in that workspace.
- `authorize` → `Decision`, then `require` / `require_type` /
  `require_account` → `Result`. The 403 reason belongs to the module
  (`inbox_read_only`, `disconnect_owner_only`, `connection_needs_user`).
- An operation declares `x-campbooks.access {resource, action}`
  (`Operation::access`).

## Data ownership and migrations

- `kernel` (`20261019130000_kernel__workspace_isolation`):
  - RLS on `changes`, `change_heads`, `blobs` and `blob_refs`: a
    transaction with no workspace set sees every row (the kernel's
    cross-workspace workers), and one with a workspace set sees only that
    workspace's rows;
  - the same on `events` and `jobs`, plus instance-wide rows;
  - `changes.audience`/`audience_user`;
  - `idempotency.containers`.

  `idempotency`, `rate_limits` and `schedules` are not keyed by workspace,
  so they have no policy. The exception is that a transaction with no
  workspace sees every kernel row: module code cannot reach that path
  (`cargo xtask arch` rejects `.begin()` and pool queries on workspace
  tables in modules), and module tables stay strict.
- `connectors` (`…130100_connectors__schedule_policies`):
  `maintenance_reads_schedule` becomes `FOR SELECT`, and
  `maintenance_leases_schedule` is a new `FOR UPDATE` policy. The dispatcher
  can no longer insert or delete schedule rows.
- `conversations` (`…130200_conversations__inbox_lifecycle`):
  `inboxes.owner` (backfilled from the managing member) and
  `inboxes.detached_at`; `removed_inboxes` (ids and a timestamp, RLS).

## Events

| Event | Change | Consumers |
| --- | --- | --- |
| `connectors.connection_removed` v1 | + `granted_by`, `provider`, `account_subject` (opaque provider id, for identity to drop connect-only ways in), `mail` (optional, default `delete`); contract file added | `connectors` → `conversations.forget_connection` (new) |
| `workspaces.member_left` v1 | + `mail` (optional, default `delete`) | `connectors.revoke_departed_grants` (passes `mail` on), `conversations.forget_member` (new) |
| `conversations.inbox_changed` v1 | new: a membership ended, the owner left, or the inbox was kept read-only | change feed only (allowlisted) |
| `conversations.inbox_removed` v1 | new | `search.on_inbox_removed` (new) |
| `conversations.send_requested`, `draft_sent`, `send_failed` v1 | + `author` (optional; contract updated) | as before |

Additive optional fields keep v1 (contracts/README: consumers ignore
unknown fields).

## Scopes and endpoints

No new operation. These change:

- `connectors.disconnect` (`connectors:write`): adds `?mail=` and is
  owner-only.
- `workspaces.remove_member` (`workspaces:admin`): adds `?mail=`.
- `conversations.list_inboxes` (`conversations:read`): adds `read_only`.

## Change feed

- Audience: `Audience::{User, UserAndManagers, Clients}` on a `Change`.
  Drafts reach their author, or the client tokens for a client's draft.
  Connections reach the grantor, owners, admins and clients, as the API
  shows them.
- Visibility is asked once per container per page, never cached across
  pages. A container shown before and hidden now sends `reset`
  (`access_changed`). Membership is re-resolved before a page with an
  access record is shown. The `Visibility` port names the record types that
  change container access (`conversations.inbox`), and so does every
  heartbeat.

## Idempotency

The handler notes each container it checked
(`idempotency::checked_container`, called by `authorize` automatically).
The list is stored with the response. A replay checks the operation's
scopes (else 403), the workspace membership and client binding, and each
container (else the handler's 404). Only then is the stored response
served.

## Purge and keep

- Delete, in one transaction per removed connection:
  1. Detach the blobs of messages, attachments, drafts and the inbox's
     batch hold with `BlobStore::detach_owners`. The sweeper deletes them
     after the grace period.
  2. Delete the inbox. Foreign keys cascade to everything else.
  3. Write a tombstone, then publish `inbox_removed`.

  A late batch or a replayed grant for a tombstoned inbox is dropped.
- Keep: set `detached_at`, which freezes the inbox in the catalogue and
  stops any further batch from applying.
- A member's departure deletes their inbox memberships and drafts and
  clears their ownership. A returning member gets nothing back.

## Lints

`cargo xtask arch` (in `xtask/src/access.rs`):

- every operation declares a catalogued resource and action;
- every workspace or account operation's handler file calls the check, or
  a listed helper;
- no `.begin()`, and no pool query naming a workspace table, in module
  code;
- every published event has a consumer, or a stated reason in
  `architecture.toml`.

**The event workspace must equal the transaction's workspace.** This is
enforced at runtime by `publish` and by RLS, not by a lint. Static analysis
cannot see which workspace a transaction was opened for, but both the
explicit check and the policy fire on every publish.

## Alternatives considered

1. **Authorization: per-module checks, RLS only, a policy engine
   (Cedar/Oso/OpenFGA), or a catalogue plus a central check.** Covered in
   ADR 0017. We chose the catalogue: no new engine or language, nothing more
   for self-hosters to run, and one source of truth per module.
2. **Change-feed revocation:**
   - (a) cache container visibility and clear it on access-record changes
     only;
   - (b) ask per page and on heartbeat;
   - (c) re-subscribe clients.

   (a) leaks during the sequencer's lag. (c) needs client work and still
   leaks until reconnect. We chose (b), with (a)'s trigger kept for an
   immediate `reset`.
3. **Private records in the feed:**
   - (a) a container per owner, decided by a module port;
   - (b) a kernel-level audience on the change;
   - (c) a separate event type per record.

   (a) needs a registry of ports and breaks for deleted records (no row
   left to ask about). We chose (b): it is decided from the caller's
   `Access` alone, needs no new machinery in modules, and works after the
   record is gone.
4. **Kernel RLS:**
   - (a) a policy that admits every row when no workspace is set (kernel
     machinery);
   - (b) a separate `campbooks.kernel` setting that every kernel query must
     set.

   (b) is stricter but touches every kernel query and the publish path of
   every module. We chose (a): the pool and `.begin()` lint covers the
   remaining gap in module code. The security review found that gap too wide;
   this change makes `publish` refuse workspace events in such a
   transaction, and (b) is campbooks-cloud#129.
5. **Idempotent replay:**
   - (a) re-run the handler;
   - (b) refuse replays entirely;
   - (c) store what the first run checked and check it again.

   (a) breaks idempotency (a second send). (b) breaks retries. We chose
   (c).
6. **Keep on disconnect:**
   - (a) move kept mail to a separate archive store;
   - (b) freeze the inbox in place.

   (a) duplicates data and needs a new reader. (b) reuses everything, with
   one flag.
7. **Clients:**
   - (a) a blanket grant to the workspace's client tokens;
   - (b) a client acts for the member who registered it.

   The security review rejected (a): a client would read inboxes its
   creator cannot, and outlive them. We chose (b): the issuer answers
   whom a client acts for (`ClientDirectory`), the caller then has that
   member's access, and a departed member's clients are removed and
   refused (401).

Nothing here copies the Rails app on `main`. It checked access per
controller (`before_action`) and kept mail on disconnect.
