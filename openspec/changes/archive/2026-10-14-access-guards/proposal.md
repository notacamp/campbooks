# Access guards: one ownership model, purge on disconnect, structural checks

Tracking: https://github.com/notacamp/campbooks-cloud/issues/117 (batch 1)

## Why

The 2026-10-13 security audit, and the owner's decisions recorded on the
issue:

- Who owns what, and who else may do what, is decided in a different place
  in each module.
- Disconnecting a mailbox leaves its mail behind.
- A departing member's inbox memberships survive their departure.
- An admin can disconnect someone else's mailbox.
- The change feed keeps sending inbox ids to a member taken out of the
  inbox, and sends connection and draft changes to everyone.
- An idempotent replay skips the access checks.
- The kernel's own tables have no row-level security.
- A search cursor is not bound to its workspace.

Nothing in CI fails when a new route forgets a check.

## What changes

- **Kernel** (`campbooks-kernel`): a resource catalogue and one central
  check, `authz::authorize` (ADR 0017), declared per module in
  `Module.resources` and answered by each module's facts port. Operations
  declare `x-campbooks.access`. A change-feed audience (`Change::for_audience`)
  is added, and streams ask about visibility on every page, sending
  `reset`/`access_changed` when something goes out of sight. A replay is
  served only after the scope, membership and container checks pass again.
  The kernel's workspace tables get row-level security, `publish` refuses an
  event of another workspace, and `BlobStore::detach_owners` is added.
- **connectors**: only the member who connected a mailbox disconnects it.
  `?mail=delete|keep` (default `delete`) travels on
  `connectors.connection_removed` (new contract) with `granted_by`, also
  from member departure. Connection changes reach the grantor, owners and
  admins, and client tokens only. The dispatcher's maintenance policy is
  narrowed to `FOR SELECT` plus `FOR UPDATE`.
- **conversations**: on `connection_removed`, the mail is deleted (inboxes
  with their messages, conversations, drafts, decisions, labels,
  corrections, memberships, held files, and a tombstone) or kept read-only
  (`detached_at`). On `member_left`, the member's inbox memberships and
  drafts are deleted and their inbox ownership cleared. New events
  `conversations.inbox_changed` and `conversations.inbox_removed`. Draft
  changes reach their author only. Every handler is moved onto
  `authorize`. Inboxes get an `owner`, and `read_only` in the inbox list.
- **workspaces**: every handler is moved onto `authorize`, and
  `remove_member` takes `?mail=keep`.
- **search**: forgets a removed inbox. Cursors are bound to the workspace,
  and `indexing` is shown only to callers who see a container.
- **identity, system, admin, server (changes, BFF)**: register their
  resource types. Identity's bearer handlers call `require_account`.
- **xtask**: `cargo xtask arch` checks access declarations, `authorize`
  calls, pool or `.begin()` queries on workspace tables, and events with no
  consumer. Exceptions are reviewed in `architecture.toml`.
- **Tests**: an access matrix generated from the catalogue and from
  `openapi.json`, plus tests of purge and keep, departure, feed
  revocation, replays and kernel isolation.

## Non-goals

- Moving identity's account resources, the BFF's session and proxy
  operations, and workspace-bound third-party tokens onto per-instance
  facts. They are registered now, and that move is PR B of the same issue.
- Content flows (batch 2) and identity hardening (batch 3) of #117.
- A way to delete a kept inbox later, or to reattach one to a reconnected
  mailbox (follow-up).
- Per-workspace data keys and crypto-erasure (#57).
