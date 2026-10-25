# Track the workspaces module's changes

Module: `workspaces` (its API operations, its admin operations, its event
handlers and its purge job), with the tracking plan (`tracking/`). Tracking:
https://github.com/notacamp/campbooks-cloud/issues/328, part of epic
https://github.com/notacamp/campbooks-cloud/issues/319.

## Why

ADR 0020 asks for a record of every action: who did it, in what context,
and what the records it touched looked like then. Workspaces is where
tenancy changes: a workspace is created, renamed, deleted and finally
purged; people are invited, join, change role, leave or are removed; a
suspended account is paused in every workspace it belongs to; and an
administrator decides who may sign up. None of it is tracked yet, while the
conversations and identity Coverage rows already point at
`workspaces.member_left`, `workspaces.member_suspended`,
`workspaces.member_unsuspended` and the workspace's deletion as tracked
"there". Full coverage becomes a CI failure with campbooks-cloud#332, so
every operation, handler and job of the module needs a Coverage row first.

## What Changes

- Tracking plan version 5 (`tracking/patches/0005-workspaces.yaml`): 16 new
  live events of `workspaces`, the snapshot records `workspaces.workspace`,
  `.member`, `.invitation`, `.signup_request` and `.signup_code`, their
  enums, and a Coverage row for each of the module's 12 mutating operations
  (8 in the API document, 4 in the admin document), its 2 event handlers and
  its purge job.
- The Coverage exemptions of conversations and identity that name the
  deletion's domain event `workspaces.workspace_deleting` now name the
  tracked event `workspaces.workspace_deleted` (an action is a past
  participle in the plan).
- Every live event is tracked with `Tracker::track` in the transaction of
  its change, after the change, from the module's own rows read in that
  transaction; a no-op tracks nothing.
- The module's test kit can write the outbox and runs its admin routes in
  the request tracking context, as the server mounts them.
- A test per event reads it back from `kernel.tracked_events`.

No new endpoint, scope, migration, dependency or domain event. No change to
what any operation returns.

## Non-goals

- The other modules (#325–#327, #329–#331) and the hard coverage gate
  (#332).
- A user's row created on their first request (`ensure_user`) and kept in
  step with their token's name and address: a mirror of the identity
  account, whose creation identity tracks (`identity.account_created`).
- Refused requests that change nothing (an invitation expired, used, for
  another address; the last owner leaving): the caller gets the error.
- The operator's `purge-again` command after a restore: a CLI step outside
  any operation, job or handler.
- Any personal data or content in events: no workspace name, no address
  (an invitee's or a member's), no person's name, no signup code or its
  label, no rejection reason, no invitation token or link.
