## ADDED Requirements

### Requirement: Rule runs and the changes they record are bound
A run of a taught rule SHALL carry a MAC under the binding key (derived from
the instance secret, never stored in the database, logged or sent), with its
own context label (`campbooks/conversations.rule-run/v1`), over the
workspace, the run, the rule, the rule's binding generation, the kind (apply
or revert), the rule's inbox and the member who asked for it. Only the
authenticated member's request that starts the run SHALL sign it, right
after it signed or verified the rule; an apply run SHALL be signed only
while its rule is bound. No job, consumer or startup task SHALL create or
sign a run. Before each batch, `conversations.apply_rule` SHALL verify the
run's MAC against the run and the rule as stored, at the rule's generation
then, in constant time; a run that does not verify, has no MAC, or whose
rule was signed again since it started SHALL change nothing and ask the
provider for nothing, and SHALL end `cancelled` with a WARN line carrying
ids only and no retry.

What a rule changes on a conversation (its application, with what the
conversation was before and what the rule did) SHALL carry a MAC with its
own context label (`campbooks/conversations.rule-application/v1`) over the
workspace, the rule, the rule's inbox, the conversation, the run that made
it (if any) and a canonical encoding of what it records. Only the path that
carries out a verified rule's effects (in a run or on arriving mail) SHALL
sign it, over what that path did and, when the conversation already held
changes of the rule, what the existing row recorded only if that row
verifies; an existing row that does not SHALL be dropped. The row SHALL be
claimed and locked before it is read (a row another transaction is
inserting SHALL be waited for), and its record and MAC SHALL be written
together from a value computed by the server, never one SQL produced. Putting a
rule's changes back SHALL verify each application under its row lock and
SHALL leave one that does not verify, or does not decode, as it is, with a
WARN line carrying ids only, and go on with the others. A row that does not
verify or decode SHALL never fail ingest or a job.

#### Scenario: A planted undo
- **WHEN** a transaction of the runtime role in a workspace inserts, for a member's signed rule, an application saying the rule reported a mail as spam (with no MAC or a made-up one), a revert run and its job
- **THEN** the run is cancelled before it changes anything, the mail stays in spam, and no `conversations.changes_requested` is published

#### Scenario: A planted apply run of a member's rule
- **WHEN** Sam taught "report GitHub as spam" without applying it to existing mail, and the runtime role plants an apply run for it (with no MAC, a made-up one, or another run's MAC copied) and its job
- **THEN** the runs are cancelled, the mail already there is left as it is, nothing is requested from the provider, and the rule still acts on new mail

#### Scenario: A run of a rule signed again
- **WHEN** Sam pauses and resumes a rule before its apply run's first batch
- **THEN** the run ends cancelled and changes nothing

#### Scenario: One application changed with SQL
- **WHEN** a rule reported three conversations as spam and SQL rewrites one application's record and copies another's, MAC included, onto the third, then Sam removes the rule with `restore=true`
- **THEN** only the untouched conversation is taken out of spam, the run ends done with one changed, and no job fails

#### Scenario: An application planted while the rule records
- **WHEN** the runtime role inserts, without committing yet, an application of a member's rule to a conversation saying the rule binned a phishing mail, while new mail the rule matches arrives in that conversation, then commits
- **THEN** the stored application records only what the rule did, signed, and the member's undo leaves the phishing mail in spam

#### Scenario: A bad row on arriving mail
- **WHEN** a rule's application to a conversation has no MAC or does not decode, and new mail the rule matches arrives in that conversation
- **THEN** ingest succeeds, the rule acts, and the application records only what the rule did now, signed

## MODIFIED Requirements

### Requirement: Undoing a rule
`conversations.remove_rule` with `restore=true` SHALL cancel any apply run
of the rule and start a revert run that puts every changed conversation back
as it was before, except those excepted or changed since (by a member, or
moved on by another rule), with the same batching, progress and events as an
apply run. Every path that locks a rule's runs and the rule SHALL take the
runs first, as a run's batch does, so undoing never deadlocks against a
batch. Only a conversation whose application verifies SHALL be put back
(see "Rule runs and the changes they record are bound"). A rule that still
holds an application with no MAC (recorded before applications were bound)
SHALL NOT be removed with `restore=true`: the request SHALL answer 409
`undo_unavailable` before anything changes, and removing it without
`restore` SHALL still work.

#### Scenario: Undo all
- **WHEN** Ana presses Z right after 998 conversations were moved
- **THEN** the rule is removed and the 998 are back in Primary, except one Leo has since starred and moved himself

#### Scenario: Changes from before the upgrade
- **WHEN** a rule holds a change recorded before applications were bound and Sam removes it with `restore=true`
- **THEN** the request answers 409 `undo_unavailable`, the rule stays active, and removing it without `restore` succeeds
