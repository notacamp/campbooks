## MODIFIED Requirements

### Requirement: Who may teach
Rules SHALL be the access catalogue's `conversations.rule`, inside their
inbox. Previewing, creating and confirming a rule, and asking for or
dismissing a suggestion, SHALL take the catalogue's `write` on the inbox (a
`sender` or a `manager`; a `reader`, an owner without a membership, or anyone
in an inbox kept read-only, gets 403 `inbox_read_only`). A proposed rule does
not act yet: confirming it and turning it down (`POST …/rules/{rule_id}/decline`,
`conversations.decline_rule`) SHALL take the same `write` on the inbox, and
confirming SHALL keep its author (the confirmer becomes it only when it had
none). Updating, removing and restoring an active rule SHALL take its author
while a `sender`, or a `manager` of the inbox; other writers get 403
`rule_not_yours`. Reading rules SHALL take any role on the inbox; a
rule in an inbox the caller cannot see SHALL answer 404. A client token, even
one a member registered (which arrives as that member), SHALL only read and
preview rules: creating, confirming, changing, removing or restoring one
answers 403 `rule_needs_member`. A rule SHALL act for every member of its
inbox. When its author no longer may write in the inbox, or the inbox is kept
read-only, the rule SHALL be paused (and its running runs stop) until a
manager adopts or removes it. While its author's account is suspended, the
rule SHALL be paused the same way (a running apply run is cancelled at its
next batch, new mail is not changed by it, and previews and suggestions do
not count it), and lifting the suspension SHALL make it act again with
nothing to redo; a run it cancelled is not restarted.

#### Scenario: Leo's rule in a shared inbox
- **WHEN** Leo taught a rule in info@ and Ana, a sender there, tries to remove it
- **THEN** the response is 403 `rule_not_yours`, and Ana's manager colleague can remove it

#### Scenario: The author leaves
- **WHEN** Leo is removed from info@
- **THEN** his rules there are paused and stop acting on new mail

#### Scenario: The author is suspended, then the suspension is lifted
- **WHEN** an administrator suspends Leo, who taught a rule in info@, and later lifts the suspension
- **THEN** while suspended the rule reads as paused with `paused_reason` `author_suspended` and leaves new mail alone, and once lifted it is active and acts on new mail again

#### Scenario: A run going when the author is suspended
- **WHEN** Leo's rule is being applied to existing mail and Leo is suspended before its next batch
- **THEN** that batch cancels the run and changes nothing

#### Scenario: An app
- **WHEN** a client token with `conversations:write`, registered by Sam, creates a rule
- **THEN** the response is 403 with code `rule_needs_member`

#### Scenario: Kept read-only
- **WHEN** the connection feeding an inbox is removed with its mail kept while a rule's run is going
- **THEN** the run is cancelled, the rule reads as paused, and creating a rule there answers 403 `inbox_read_only`

### Requirement: Purging rules
Removing an inbox's mail and purging a workspace SHALL delete its rules,
runs, applications and dismissals; purging a workspace SHALL also delete the
suspensions it recorded for its members. Keeping an inbox read-only SHALL
cancel its rules' runs and drop what only served acting again: the members'
words (`origin_text`), what each rule changed (`before`) and the dismissed
suggestions. A member leaving SHALL drop the suggestions they dismissed.

#### Scenario: Disconnecting with the mail deleted
- **WHEN** the inbox's connection is removed and its mail deleted
- **THEN** its rules are gone too

## ADDED Requirements

### Requirement: Suspended authors
The module SHALL record, per workspace, whether each member is suspended,
from `workspaces.member_suspended` and `workspaces.member_unsuspended`
(`conversations.pause_suspended_author`,
`conversations.resume_unsuspended_author`), keeping for each member the
change with the newest `at`: a duplicate, or a change older than the one
recorded, SHALL change nothing. When the record changes, the module SHALL
publish `conversations.rule_changed` for each of the member's rules that is
not removed. `RuleView` SHALL carry `paused_reason` when its state is
`paused`: `inbox_read_only`, else `author_cannot_write`, else
`paused_by_member`, else `author_suspended`.

#### Scenario: Delivered out of order
- **WHEN** the lift (newer `at`) is delivered before the suspension it lifts
- **THEN** the member is recorded as not suspended and their rules stay active

#### Scenario: Delivered twice
- **WHEN** the same suspension is delivered twice
- **THEN** the member is suspended once and `rule_changed` is published once per rule
