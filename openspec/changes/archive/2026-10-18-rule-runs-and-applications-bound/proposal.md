# Proposal

## Why

The review of public #556 (campbooks-cloud#242, items 8 and 9) left two gaps
open on purpose. An attacker with SQL as the runtime role inside workspace
W, but not the instance secret, can still:

- **(8, MEDIUM)** plant a `conversations.rule_applications` row whose
  `before` says "the rule put this in spam", plus a revert run and its
  `conversations.apply_rule` job, against any rule (a member's real one
  will do). `revert_batch` then asks Gmail or Zoho to take a phishing mail
  out of spam or the trash;
- **(9, LOW)** plant a `conversations.rule_runs` apply run and its job for a
  member's real, signed rule, so it trashes or archives old mail the member
  taught it to leave alone (`apply_to_existing=false`).

After this change both change nothing, and no job signs a run or an
application whose content SQL controls.

Modules touched: `conversations` only. One migration (three nullable
columns). No new endpoint, event, scope, dependency or secret; one new
problem code (`undo_unavailable`, a 409 of `conversations.remove_rule`).

## What Changes

- **A run carries a MAC** (`rule_runs.mac`, label
  `campbooks/conversations.rule-run/v1`) over the workspace, the run, the
  rule, the rule's binding generation, the kind, the rule's inbox and the
  member who asked for it (`rule_runs.requested_by`). Only the member's
  request that starts it signs it (`rule_runs::start`), right after that
  request signed or verified the rule. `conversations.apply_rule` verifies
  it, at the rule's generation now, before counting or changing anything;
  a run that does not verify ends `cancelled` with a WARN carrying ids only,
  and is not retried.
- **An application carries a MAC** (`rule_applications.mac`, label
  `campbooks/conversations.rule-application/v1`) over the workspace, the
  rule, the rule's inbox, the conversation, the run and its `before` (what
  the conversation was and what the rule did). Only the effect path signs it
  (`effects::record`), right after the rule verified; when the conversation
  already held changes of the rule, what that row recorded is kept only if
  it verifies too. Putting back verifies each row under its lock and leaves
  one that does not as it is (a WARN with ids), going on with the rest.
- **Rows from before the deploy are not signed.** A run going at the deploy
  stops at its next batch. What a rule changed before cannot be put back:
  `conversations.remove_rule` with `restore=true` answers 409
  `undo_unavailable` for a rule holding such a change; removing it keeping
  its changes still works.

## Non-goals

- **Replaying what a member's request left.** A run or an application row,
  MAC included, can be restored by SQL exactly as it was (the limit of
  campbooks-cloud#227 §8 and of the rules' own MACs).
- **SQL-writable state the effect path reads** (a conversation's stream and
  who placed it, its messages' provider ids, labels): rewriting it changes
  what a rule's change, or its undo, names. That is item 4 of
  campbooks-cloud#242.
- **A screen for `undo_unavailable`**: the client shows its usual "the rule
  did not change" message; offering "keep its changes" there is later work.

## Impact

- SQL in a workspace can no longer make a rule's undo, or a run of a rule,
  change mail at the provider.
- A run of a rule that a member changes, pauses and resumes, or adopts while
  the run is going stops at its next batch.
- At the deploy, running runs stop, and rules that changed mail before it
  cannot have those changes put back automatically.
