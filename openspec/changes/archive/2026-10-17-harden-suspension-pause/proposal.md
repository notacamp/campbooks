# Proposal

## Why

The security review of the pause-on-suspension change (campbooks-cloud#147,
notacamp/campbooks#513) left five non-blocking findings, tracked in
https://github.com/notacamp/campbooks-cloud/issues/197:

1. A suspension and its lift are ordered by the database clock. If the clock
   steps back between the two, the lift loses and the rules stay paused until
   the next change (fails closed).
2. A member who re-joins a workspace at the moment they are suspended can
   miss the suspension there (fails open, narrow).
3. A member removed at the moment they are suspended never hears the later
   lift in that workspace, so after a re-invite their rules stay paused until
   adopted (fails closed).
4. The pause rides two hops of background events. If either delivery dies
   after its retries, nothing tells an operator, and a suspended member's
   rules keep acting.
5. Keeping a proposed rule whose author is away gives no warning that it
   stays paused.

## What Changes

- **identity**: `identity.accounts.suspension_seq`, the account's count of
  suspensions and lifts, bumped by one under the account's row lock on every
  change. `identity.account_suspended` and `identity.account_unsuspended`
  carry it as `seq` (additive; events from before it have none).
- **workspaces**: the fan-out orders changes by `seq` (falling back to `at`
  for events without it) and keeps the newest one on the user's row
  (`workspaces.users.suspended`, `suspension_seq`, `suspension_at`), read and
  written under that row's lock. Accepting an invitation, creating a
  workspace and leaving or being removed take the same lock first. Joining
  announces the newest change in the workspace joined (creating one only
  when the user is suspended), so whichever commits first, every workspace
  the user belongs to hears the newest change. `workspaces.member_suspended`
  and `workspaces.member_unsuspended` carry `seq`.
- **conversations**: `member_suspensions.seq`; the higher `seq` wins, a
  change without one is older than any with one, and two without are ordered
  by `at` as before. A departure no longer rewrites a counted change.
  `RuleView.paused_once_kept` says, for a proposed rule, why it would be
  paused once kept.
- **kernel**: `jobs::Watch`, declared on a job or event handler (`WATCH`): a
  dead job of a watched kind is logged at WARN with a stable line and
  counted per code in `GET /admin/v1/system/jobs` (`watched`). The four
  suspension handlers are watched as `suspension_fanout`, logging
  "suspension fan-out delivery died".
- **Clients**: Admin → Jobs shows "Work to watch" with the dead deliveries of
  each code; the rules list says, before Keep this rule, that a proposal
  whose author is away stays paused once kept, and the toast after keeping
  says it is paused for now. In en, pt-PT, es-ES and fr.

## Non-goals

- No change to who may suspend, or to what a suspension ends.
- No alert rule in the hosted cloud's monitoring: the WARN line is what one
  can match later.
- No retry of dead deliveries on its own: an operator retries them from
  Admin → Jobs, as for any job.
- No new module dependency, no new kernel-scope access.

## Impact

- Migrations: `20261021090000_identity__suspension_seq`,
  `20261021090100_workspaces__member_suspension_state`,
  `20261021090200_conversations__member_suspension_seq` (columns with
  constant defaults, no rewrite; `SET LOCAL lock_timeout`).
- Events (additive, same versions): `seq` on `identity.account_suspended`,
  `identity.account_unsuspended`, `workspaces.member_suspended`,
  `workspaces.member_unsuspended`.
- API (additive): `RuleView.paused_once_kept`; admin `QueueSummary.watched`
  (`WatchedSummary`).
