# Design

How to add a tracked event is in `docs/tracking.md`; the decision is
ADR 0020. This change applies it to the whole `workspaces` module.

## Data ownership and events

Every snapshot is read from `workspaces`' own rows, in the transaction of
the change: `workspaces.workspaces`, `.members` (with the member's
`users.suspended`), `.invitations`, `.signup_requests` and `.signup_codes`.
New queries are runtime `sqlx::query_as`; no query the module already has
changes its text. Events go to `kernel.tracked_events` (the kernel's
outbox); nothing else is written. No domain event is published or consumed
that was not before, no endpoint, scope or AI role changes.

What an event may hold: ids, enums, booleans, counts and timestamps. Never
a workspace's name, an address (an invitee's, a member's), a person's name,
a signup code or its label, a rejection reason, an invitation token or its
link. An invitation is named by its id, and its invitee not at all; a
member by their user id (`pseudonymous`); an administrator only by the
envelope's `admin` actor. Which settings an update changed is an enum
(`name`, `accent`), never their values; the accent itself is an enum of
presets and is in the snapshot.

**Scopes.** Workspace events are written from the transaction working in
that workspace (`begin_for`, the purge job's `tx()`). Signup requests and
signup codes belong to no workspace, so their events are instance-wide,
written from the user's or the administrator's transaction; minting and
revoking a code now run in an administrator's transaction (`begin_admin`,
as deciding a request does; they ran on the pool) so the event commits with
the change. A code is redeemed in the
transaction that creates the workspace, where the tracker refuses an
instance-wide event, so `signup_code_redeemed` is workspace-scoped: the
workspace the code opened.

## Actors

The API operations are `user` (every one of them needs a person:
`acting_user`, `ensure_user`); the admin operations are `admin`; the two
suspension handlers and the purge job are `system`, on behalf of whoever
caused them (the administrator who suspended the account, the owner who
deleted the workspace).

## Records (snapshots)

| Record | Fields |
| --- | --- |
| `workspaces.workspace` | `id`, `kind` (`business`, `personal`), `accent` (`business`, `personal`, `forest`), `member_count`, `created_at`, `deleted_at`?, `purge_after`? |
| `workspaces.member` | `user_id`, `role` (`owner`, `admin`, `member`), `suspended`, `joined_at` |
| `workspaces.invitation` | `id`, `role`, `state` (`pending`, `accepted`, `expired`), `invited_by`, `created_at`, `expires_at`, `accepted_at`?, `accepted_by`? |
| `workspaces.signup_request` | `user_id`, `status` (`pending`, `approved`, `rejected`), `with_reason`, `requested_at`, `decided_at`? |
| `workspaces.signup_code` | `id`, `state` (`active`, `used_up`, `expired`, `revoked`), `max_uses`, `uses`, `expires_at`?, `created_at`, `revoked_at`? |

`?` marks an optional field. `user_id`, `invited_by` and `accepted_by` are
`pseudonymous` (a workspaces user id); the records' own ids are `id`.
`with_reason` says whether an administrator wrote a reason, never what.

## Events

All are version 1, `live`, added in plan version 5 by this change.

### Workspaces (workspace, `user` unless said)

| Event | Fires when | Properties |
| --- | --- | --- |
| `workspace_created` | `workspaces.create` creates a workspace | `workspace` |
| `workspace_updated` | `workspaces.update` changes its name or accent; the same values again track nothing | `workspace`, `fields` (list of `name`, `accent`) |
| `workspace_deleted` | `workspaces.delete` deletes it (hidden at once, purged after the grace period) | `workspace` (with `deleted_at`, `purge_after`) |
| `workspace_purged` | the `workspaces.purge_workspace` job hard-deletes it at the end of its grace period (`system`); a workspace gone already tracks nothing | `workspace` (as it stood when purged) |

### Members (workspace)

| Event | Fires when | Properties |
| --- | --- | --- |
| `member_joined` | a person becomes a member: the creator of a workspace (`workspaces.create`, via `created`) or an invitee accepting (`workspaces.accept_invitation`, via `invitation`) | `member`, `via`, `invitation`? (accepted) |
| `member_role_changed` | `workspaces.change_role` gives a member another role; the same role tracks nothing | `member`, `before` (role) |
| `member_left` | a member leaves (`workspaces.leave`, or `workspaces.remove_member` on oneself: `left`) or is removed (`workspaces.remove_member`: `removed`) | `member` (as it stood), `how` (`left`, `removed`), `mail` (`delete`, `keep`) |
| `member_suspended` | `workspaces.announce_member_suspended` announces an account's suspension in a workspace its user belongs to (`system`); a change that leaves them as they were (a replay, an older change) tracks nothing | `member` |
| `member_unsuspended` | `workspaces.announce_member_unsuspended` announces the lift, likewise | `member` |

### Invitations (workspace)

| Event | Fires when | Properties |
| --- | --- | --- |
| `invitation_created` | `workspaces.invite_member` invites someone; inviting the same address again replaces the pending invitation | `invitation`, `replaced` (list of the replaced invitations' ids) |

### Signups (instance; `signup_code_redeemed` workspace)

| Event | Fires when | Properties |
| --- | --- | --- |
| `signup_request_filed` | `workspaces.create` under the `approval` policy files a person's first request (`user`); asking again tracks nothing | `signup_request` |
| `signup_request_approved` | `workspaces.admin_approve_signup_request` approves a request that was not (`admin`) | `signup_request`, `before` (status) |
| `signup_request_rejected` | `workspaces.admin_reject_signup_request` rejects a request that was not (`admin`) | `signup_request`, `before` (status) |
| `signup_code_minted` | `workspaces.admin_mint_signup_code` mints a code (`admin`) | `signup_code` |
| `signup_code_revoked` | `workspaces.admin_revoke_signup_code` revokes a code that was not (`admin`) | `signup_code` |
| `signup_code_redeemed` | `workspaces.create` spends a use of a minted code (`user`, in the workspace it opens) | `signup_code` |

Not tracked as their own events, on purpose:

- The invitation's acceptance: it is the `member_joined` that carries the
  accepted invitation, one event for one act.
- The owner's membership of a new workspace is its own `member_joined`
  (`via: created`), as the domain events have it, so a workspace's members
  are always the sum of its joins and departures.
- A suspension announced in a workspace on joining it: the member's
  snapshot in `member_joined` says `suspended`.
- `member_left`'s snapshot is the membership as it stood just before it
  ended, read in the same transaction (afterwards there is no row).

## Coverage

Operations (`openapi/openapi.json`, tag `workspaces`; `admin.json` for the
`admin_*`):

| Operation | Events |
| --- | --- |
| `create` | `workspace_created`, `member_joined`, `signup_code_redeemed`, `signup_request_filed` |
| `update` | `workspace_updated` |
| `delete` | `workspace_deleted` |
| `invite_member` | `invitation_created` |
| `accept_invitation` | `member_joined` |
| `change_role` | `member_role_changed` |
| `remove_member` | `member_left` |
| `leave` | `member_left` |
| `admin_approve_signup_request` | `signup_request_approved` |
| `admin_reject_signup_request` | `signup_request_rejected` |
| `admin_mint_signup_code` | `signup_code_minted` |
| `admin_revoke_signup_code` | `signup_code_revoked` |

Handlers:

| Handler | Events |
| --- | --- |
| `announce_member_suspended` | `member_suspended` |
| `announce_member_unsuspended` | `member_unsuspended` |

Jobs:

| Job | Events |
| --- | --- |
| `purge_workspace` | `workspace_purged` |

The module has no schedule. Its reads need no row.

Rows of other modules that name this module's events, updated by this
patch where the name changed:

| Row | Exemption |
| --- | --- |
| handler `conversations.forget_member` | consequence of `workspaces.member_left` (unchanged) |
| handler `conversations.pause_suspended_author` | projection of `workspaces.member_suspended` (unchanged) |
| handler `conversations.resume_unsuspended_author` | projection of `workspaces.member_unsuspended` (unchanged) |
| handler `conversations.schedule_workspace_purge` | projection of `workspaces.workspace_deleted` (was `workspace_deleting`, the domain event) |
| handler `identity.remove_workspace_clients` | consequence of `workspaces.workspace_deleted` (was `workspace_deleting`) |

## Notes

- The purge job tracks `workspace_purged` in the transaction that deletes
  the workspace's row, so the event exists if and only if the purge
  committed. Like everything of a purged workspace, the outbox row goes
  with the kernel's next sweep of tombstones; with the sink `amqp` the
  relay has published it by then.
- Suspension announcements are delivered in a transaction per workspace,
  and a change announced again (a replay, or a retry after a partial
  failure) is published again for consumers to ignore. It is tracked only
  when it changes whether the user is suspended (against the change
  recorded on the user's row), so a replay tracks nothing; a retry after a
  partial failure may track a workspace twice (at least once, as the relay
  is).
- Revoking a code and deciding a request are idempotent; they now say
  whether anything changed, and only a change is tracked. An update that
  sets the values a workspace already has tracks nothing.
- Patch numbers and plan versions are taken in merge order; this one builds
  on plan version 4 (`0004-bff`).
