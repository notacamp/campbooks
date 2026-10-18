## ADDED Requirements

### Requirement: Taught rules are bound to the member who taught them
A taught rule SHALL carry a MAC under the binding key (the key derived from
the instance secret, never stored in the database, logged or sent), with its
own context label (`campbooks/conversations.rule/v1`), over the workspace,
the rule, its binding generation (one more at every signing), its inbox, who
taught it, its stored state, its precedence (`confirmed_at`, else
`created_at`) and a canonical hash of its conditions and actions (each
length-prefixed, in order). Only an authenticated member's request SHALL
sign a rule: creating it active, confirming it, changing it, pausing or
resuming it, adopting it, removing it and bringing it back; a proposal SHALL
NOT be signed until a member confirms it. A request that keeps a bound field
it does not set SHALL verify the rule under its row lock first and SHALL NOT
sign a rule that does not verify: such a rule MAY be paused, and any other
change SHALL answer 409 `rule_needs_confirmation`. No job, consumer or
startup task SHALL sign a rule whose content SQL can set. Without an
instance secret a rule SHALL NOT be saved active (500).

Before a rule acts (on arriving mail, in a run over existing mail, or to
show a sender's images) its MAC SHALL be verified against the rule as
stored, in constant time. A rule that does not verify, or has no MAC, SHALL
change nothing, at the provider or locally, and ask the provider for
nothing, with a WARN line carrying ids only and no retry; an apply run of it
SHALL end cancelled. The other rules SHALL still act, and a rule that does
not verify or does not decode SHALL never fail ingest.

A kept rule (active or paused) that does not verify SHALL be listed with
`needs_confirmation: true`; `conversations.confirm_rule` SHALL confirm it
again: anyone who may teach in its inbox, signing it as it is, in its state
and with its author. A rule that verifies SHALL NOT be confirmed again (409
`rule_not_proposed`).

#### Scenario: A rule planted with SQL
- **WHEN** a transaction of the runtime role in a workspace inserts an active rule "trash everything from this sender", taught by a manager, with no MAC, a made-up one or another rule's, and plants an apply run for it
- **THEN** new mail from that sender is stored and left as it is, the run is cancelled before it changes anything, no `conversations.changes_requested` is published for it, and the rule is listed as needing confirmation

#### Scenario: A signed rule changed with SQL
- **WHEN** a member's rule has its actions, conditions, inbox, author, precedence or generation changed with SQL, or a paused or removed state set back to active
- **THEN** it no longer acts, and its other changes wait for a member to confirm it

#### Scenario: A MAC copied elsewhere
- **WHEN** a rule's MAC is copied onto another rule with the same content, in the same inbox, another inbox or another workspace
- **THEN** none of them acts

#### Scenario: What members teach still acts
- **WHEN** Sam teaches a rule, changes it, pauses and resumes it, removes it and brings it back
- **THEN** it acts on the next mail and its changes are requested from the provider

#### Scenario: Confirming again
- **WHEN** a rule changed with SQL is listed as needing confirmation and Sam confirms it
- **THEN** it is bound as it is, in its state, and acts once active

#### Scenario: No instance secret
- **WHEN** the server has no `CAMPBOOKS_SECRET_KEY` and a member saves an active rule
- **THEN** the request fails with 500 and no rule acts

### Requirement: Rules kept from before rules were bound
The migration that adds the binding SHALL record each kept rule's (active or
paused) id, workspace and a SHA-256 of its bound fields, computed in SQL as
text no session setting changes, in `conversations.rule_resigns`, which the
runtime role MAY read and SHALL NOT insert, update or delete.
`conversations.take_rule_resign` (`SECURITY DEFINER`, owned by
`campbooks_definer`, `search_path = pg_catalog, pg_temp`) SHALL remove a
rule's row in the transaction's workspace and answer whether the rule as
stored is kept and hashes, with the same expression, to the recorded value.
`conversations.dispatch_rule_resigns` (every minute, the maintenance scope)
SHALL enqueue `conversations.resign_rules` in each workspace with rows; that
job SHALL sign a recorded rule only when the function answers true, never
wait on a rule a member's request holds, and take nothing without an
instance secret. A rule changed since the migration, or not recorded by it,
SHALL stay unsigned, with a WARN line carrying ids only.

#### Scenario: The owner's rules at the upgrade
- **WHEN** the migration runs over active and paused rules and the dispatcher runs
- **THEN** each rule still as it was is signed once, acts on the next mail, and leaves the snapshot

#### Scenario: Changed after the migration
- **WHEN** a recorded rule's actions are changed with SQL before the job runs
- **THEN** it is not signed, leaves the snapshot, and waits for a member to confirm it

#### Scenario: Adding to the snapshot
- **WHEN** the runtime role inserts, updates or deletes a row of `conversations.rule_resigns`, or plants `conversations.resign_rules`
- **THEN** the write is refused, and the job signs only rules the migration recorded
