# Proposal

## Why

A taught rule acts in its author's name (campbooks-cloud#125). Suspending a
member (#497) cuts their own access but leaves their workspace and inbox
memberships in place, so the rules they taught, and an apply run already
going, keep sorting mail for everyone in a shared inbox. A suspended person
can't act; neither should the rules acting for them.

Owner's decision (delegated, 2026-10-16) on campbooks-cloud#147: **pause on
suspension**.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/147

## What Changes

- **identity** publishes `identity.account_unsuspended` (instance-wide) when
  a suspension is lifted, the counterpart of `identity.account_suspended`.
  Both carry `at`, the moment the change took effect, read under the
  account's row lock so a later change always has a later `at`.
- **workspaces**, which owns the link between an account (`sub`) and its
  users and memberships, handles both and publishes, in each workspace the
  account belongs to, the workspace-scoped `workspaces.member_suspended` /
  `workspaces.member_unsuspended` with the user and the same `at`.
- **conversations** handles those and records the member's suspension in
  its own schema (`conversations.member_suspensions`, workspace-keyed, with
  row-level security), newest `at` wins. A rule whose author is suspended
  reads as **paused**, exactly like a rule whose author can no longer change
  the inbox: it stops acting on new mail, teach-once previews and offers
  stop counting it, and a running apply run is cancelled at its next batch.
  Lifting the suspension makes the rule active again with nothing to redo.
- `RuleView` gains `paused_reason` (`inbox_read_only`, `author_cannot_write`,
  `paused_by_member`, `author_suspended`), and the rules list says why a
  rule is paused, "Paused while whoever taught it is suspended" among them,
  in en / pt-PT / es-ES / fr.

## Impact

- Events: `identity.account_suspended` gains `at` (additive, optional for
  older events); new `identity.account_unsuspended`,
  `workspaces.member_suspended`, `workspaces.member_unsuspended`.
- Migration: `conversations__member_suspensions` (one small table).
- API: `RuleView.paused_reason` (additive).
- Client: the rules list's paused line names the reason.
- No new kernel-scope access and no module-to-module import.
