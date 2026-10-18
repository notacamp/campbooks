# Proposal

## Why

The review of public #552 (campbooks-cloud#227) closed sends: SQL alone can
no longer make a mailbox send. It left a HIGH gap, item 1 of
campbooks-cloud#242. An attacker with SQL as the runtime role inside
workspace W, but not the instance secret, can insert a taught rule ("match
everything from this sender, trash it", taught by any manager) or quietly
change an existing rule's conditions, actions or inbox. The next inbound mail
runs it in `rules::on_arrival`, or a planted `conversations.apply_rule` job
runs it over the mail already there, and `effects::request` signs its
changes as the server's own: Gmail or Zoho then trashes, labels or reports
the mailbox as spam.

After this change a rule planted or changed with SQL changes nothing, at the
provider or in what a member sees, and no job, consumer or startup task can
sign a rule whose content SQL can set.

Modules touched: `conversations` (signs rules in members' requests, verifies
before any rule acts, re-signs the rules kept at the deploy) and the server
(a test's expectations of the definer role). One migration. No new endpoint,
event, scope, dependency or secret; one new field (`RuleView.needs_confirmation`),
one new problem code (`rule_needs_confirmation`), two new background jobs.

## What Changes

- **A rule carries a MAC** (`rules.mac`) under the binding key of
  campbooks-cloud#227 with its own label (`campbooks/conversations.rule/v1`),
  over the workspace, the rule, its binding generation (one more at every
  signing), its inbox, who taught it, its stored state, its precedence
  (`confirmed_at`, else `created_at`) and a canonical hash of its conditions
  and actions.
- **Only a member's request signs it**: creating it active, confirming it,
  changing it, pausing or resuming it, a manager adopting it, removing it and
  bringing it back. A request that keeps bound fields it did not set first
  verifies the rule under its row lock; a rule that does not verify may only
  be paused (anything else answers 409 `rule_needs_confirmation`) and is not
  signed again. A proposal is never signed until a member says yes.
- **No rule acts unless it verifies.** `rules::acting` (the rules the engine
  sees on arrival, in a run, and for "Always show images") leaves out an
  active rule whose MAC does not verify, with a WARN carrying ids only; an
  apply run of such a rule stops before its first change. Other rules still
  act, and a rule that does not even decode is left out instead of failing
  the batch.
- **Rules kept at the deploy keep working.** The migration snapshots each
  kept rule's id and a SHA-256 of its bound fields, computed in SQL, into a
  table the runtime role may read and never write.
  `conversations.resign_rules` signs a snapshotted rule once, only when a
  `SECURITY DEFINER` function (owned by `campbooks_definer`) confirms, with
  the same SQL expression, that the rule is still as the migration found it,
  and removes it from the snapshot either way.
- **A rule that needs a yes again says so** (`RuleView.needs_confirmation`),
  and `conversations.confirm_rule` confirms it again: it signs the rule as it
  is, in its state, with the confirming member as its author.

## Non-goals

- **Replaying a state a member left.** SQL can restore a rule's row exactly
  as a member's request left it, MAC included (the same limit as
  campbooks-cloud#227's §8): a rule removed by its member can be brought back
  only by someone who read its MAC while it was kept. Closing it needs state
  outside the database.
- **The rest of campbooks-cloud#242** (draft content tokens, replay of
  sends, stream lookups, Zoho's `restore_to`, re-reads, key rotation): later
  changes.
- **What a rule's run puts back** (`rule_applications.before`) and the run
  rows themselves are not bound: a planted revert run can put back what
  `before` rows say (see `design.md`).
- **A screen for confirming again**: the flag and the operation are there;
  the rules list shows it in a later change.

## Impact

- SQL in a workspace can no longer make a mailbox change through a taught
  rule, nor show a sender's images through one.
- At the deploy, rules do not act until `conversations.resign_rules` has
  signed them (its dispatcher runs every minute); a rule changed in that
  window waits for a member to confirm it.
- Without an instance secret a rule cannot be saved active (500), as a send
  cannot be sent; one already saved does not act.
