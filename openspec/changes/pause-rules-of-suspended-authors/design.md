# Design

## Context

- Whether a rule may act is decided in one place: `rules::Stored::effective_state`
  turns an active rule into `paused` when `author_may_act` is false (its
  author is no longer a `sender` or `manager` of the inbox) or the inbox is
  kept read-only. Every path goes through it: the evaluator on new mail
  (`rules::acting`, used by `apply_changes`), each batch of an apply run
  (`apply_rule` re-reads the rule and cancels the run when it is no longer
  active), previews and teach-once offers (`compute_preview`), and the rules
  list (`RuleView.state`).
- Suspension lives in `identity` (`identity.accounts.disabled_at`), keyed by
  the account id, which is a token's `sub`. `conversations` knows members by
  `workspaces.users.id` (`UserId`), a different id; only `workspaces` maps
  one to the other (`workspaces.users.idp_subject` = `sub`).
- `identity.account_suspended` is instance-wide. Since campbooks-cloud#129 a
  transaction that sets no workspace sees no row of any table with
  row-level security, and a workspace event can only be published from a
  transaction working in that workspace.

## Decision

Suspension travels by events, and every module only touches what it owns:

1. `identity` publishes `identity.account_suspended` and the new
   `identity.account_unsuspended`, both instance-wide, carrying `at`.
2. `workspaces` handles both (`workspaces.announce_member_suspended`,
   `workspaces.announce_member_unsuspended`): it finds the user by `sub`,
   reads their memberships across workspaces through its existing
   `users_see_their_memberships` policy (the handler's transaction sets
   `campbooks.user_id`, as `begin_as_user` does), and for each live
   workspace publishes `workspaces.member_suspended` /
   `workspaces.member_unsuspended { workspace, user, at }` in a
   `begin_for(workspace)` transaction.
3. `conversations` handles those in the workspace's transaction
   (`conversations.pause_suspended_author`,
   `conversations.resume_unsuspended_author`): it upserts
   `conversations.member_suspensions (workspace_id, user_id, suspended, as_of)`
   and, when that changed something, announces `rule_changed` for each of
   the member's rules so lists refresh. The rule queries add
   `author_suspended` next to `author_may_act`, and `effective_state` pauses
   on it.

No module imports another, and nothing new needs the kernel scope.

### Alternatives considered

- **conversations handles `identity.account_suspended` directly.** It would
  need the `sub` → `UserId` mapping (owned by `workspaces`) and to write
  rows of every workspace from an instance-wide job, which needs a new
  `[[access.kernel_scope]]` entry. Rejected.
- **workspaces stores the suspension and conversations asks it.** A
  synchronous cross-module read (a port) in the hot path of every rule
  evaluation, and still a new port for one boolean. Rejected: the event's
  copy is tiny and local.
- **Copy suspension onto `conversations.inbox_members`.** Mixes a
  person-level fact into a per-inbox row and is lost when memberships are
  rewritten. A table keyed by workspace and user is clearer.
- **Remove the suspended member's inbox memberships.** Lifting the
  suspension would then not bring the rules back: rejected by the decision.

### Ordering and idempotency

Each change carries `at`. `identity` reads it with `clock_timestamp()` after
locking the account row, so of two changes to one account the later one,
which waited for the earlier to commit, always has the later `at`.
`workspaces` passes it through unchanged, and `conversations` keeps a row
per member with the newest `at` it has seen:

```sql
INSERT … ON CONFLICT (workspace_id, user_id) DO UPDATE
   SET suspended = EXCLUDED.suspended, as_of = EXCLUDED.as_of
 WHERE member_suspensions.as_of < EXCLUDED.as_of
```

A lift is kept as a row with `suspended = false`, so a suspension delivered
late (older `at`) is ignored. A duplicate changes nothing. Events from
before this change have no `at`; they fall back to the event's
`occurred_at`.

The `workspaces` fan-out commits one transaction per workspace. If it fails
part way, the retry publishes again to the workspaces already done; the
consumer is idempotent, so this is harmless.

### The running apply run

Each batch of `conversations.apply_rule` reads the rule with `find_any` and
cancels the run when its effective state is not active. Suspension feeds
that state, so the batch after the suspension commits cancels the run. A
cancelled run is not restarted when the suspension is lifted: the rule acts
on new mail again, as it does when a member resumes a paused rule or a
manager adopts one.

### Why a rule is paused

`RuleView.paused_reason` names the one reason that would still hold once
the most passing one clears: `inbox_read_only`, then `author_cannot_write`,
then `paused_by_member`, then `author_suspended`. The client words each in
the four languages. Until now a rule a member paused read as "whoever
taught it can no longer change this inbox", which was wrong.

### Accounts suspended before this ships

Suspension (#497) shipped days before this change. No migration can backfill
it (migrations touch only their module's schema, and identity's state is
not readable by conversations). Replaying `identity.account_suspended` from
the start to the new `workspaces` consumer would wrongly pause the rules of
accounts suspended and then lifted before this change, because those lifts
left no event. So:

- no automatic backfill;
- before release the operator counts suspended accounts
  (`SELECT count(*) FROM identity.accounts WHERE disabled_at IS NOT NULL`);
  if there are any, for each one they enqueue the account's newest
  `identity.account_suspended` event to `workspaces.announce_member_suspended`
  (the kernel's replay inserts the same job row; the operator inserts it
  for that one event id). Hosted-cloud steps live in the private runbook.

## Risks

- A member suspended while the `workspaces` fan-out job is queued keeps
  their rules acting for that short time; the next batch of a running run
  re-checks.
- `conversations.member_suspensions` keeps a row per member who was ever
  suspended in the workspace; it is deleted with the workspace.
