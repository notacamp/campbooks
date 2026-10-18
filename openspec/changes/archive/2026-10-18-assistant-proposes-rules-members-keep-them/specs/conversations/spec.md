## ADDED Requirements

### Requirement: Only a member's own request says yes to a rule
A rule SHALL act only through a member's own request: one that reached the
API over HTTP with the person's token (their client, or an app they
allowed). When the assistant, or an automation rule, runs a rule
operation on a member's behalf (`kernel::operations::invoke`), the server
SHALL let it propose a rule and never make one act or sign it as the
member's:

- `conversations.confirm_rule` SHALL NOT be callable by the assistant
  (`x-campbooks.assistant` false), so the kernel refuses the call before
  the handler runs, confirmed or not; and the handler SHALL refuse with 403
  `rule_needs_your_yes` any invoked request that reaches it.
- `conversations.create_rule` SHALL store what an invoked request saves as
  `proposed` whatever state, origin or member preference it carries (it is
  not signed and applies to nothing), and SHALL refuse one that answers a
  suggestion (`notice_id`, which keeps a rule acting at once) with 403
  `rule_needs_your_yes`.
- `conversations.update_rule` SHALL refuse, with 403 `rule_needs_your_yes`,
  an invoked request that adopts a rule, lets it act again (`paused:
  false`) or changes the conditions or actions of a rule that is not a
  proposal. Pausing a rule, and reshaping a proposal (which still waits for
  a yes), SHALL stay allowed.
- `conversations.restore_rule` SHALL refuse, with 403 `rule_needs_your_yes`,
  an invoked request that would bring a rule back active or paused; a
  proposal may come back, still waiting for its yes.
- `conversations.decline_rule` and `conversations.remove_rule`, which make
  a rule act less, SHALL stay allowed.

Each refusal SHALL come after the access checks and before anything is
changed or signed, with a WARN line carrying the workspace and rule ids and
the caller kind only. Whether a request was invoked SHALL be the kernel's to
say, from a request extension only `invoke` attaches; nothing a client sends
SHALL set it or clear it. A member's own yes (the rules list's Keep, with
the token of what their screen showed) SHALL keep working unchanged.

#### Scenario: Scout says yes with the member's token
- **WHEN** the assistant, acting for Sam, reads a proposed rule (its `content_token` is Sam's) and calls `conversations.confirm_rule` with that token, as a call Sam confirmed or not
- **THEN** the call is refused before the handler runs, the rule is still proposed and unsigned, no run starts and nothing is asked of the provider

#### Scenario: Sam keeps it himself
- **WHEN** Sam then keeps the proposal from the rules list with the token his screen got
- **THEN** it is active, signed, and Sam is its author

#### Scenario: Scout saves a rule asking for it to act
- **WHEN** the assistant, acting for a member who lets Campbooks act without asking, saves a rule active or from corrections
- **THEN** it is stored proposed and unsigned, `acted_without_asking` is false, and nothing changes in the inbox until the member says yes

#### Scenario: Scout answers a suggestion
- **WHEN** the assistant saves a rule with a `notice_id`
- **THEN** the answer is 403 `rule_needs_your_yes` and nothing is saved

#### Scenario: Scout adopts, resumes or rewrites a kept rule
- **WHEN** the assistant adopts Sam's active rule with Sam's token, changes its actions, sets its whole content, or resumes it once paused
- **THEN** each answers 403 `rule_needs_your_yes` and the rule's binding generation, MAC and state are as before

#### Scenario: Scout pauses, removes and reshapes
- **WHEN** the assistant pauses Sam's rule, removes it, or changes the actions of a proposal
- **THEN** each succeeds; bringing the removed rule back is refused (403 `rule_needs_your_yes`) until Sam's own Undo brings it back acting, and the reshaped proposal waits for Sam's yes to what it now does

## MODIFIED Requirements

### Requirement: Who may teach
Rules SHALL be the access catalogue's `conversations.rule`, inside their
inbox. Previewing, creating and confirming a rule, and asking for or
dismissing a suggestion, SHALL take the catalogue's `write` on the inbox (a
`sender` or a `manager`; a `reader`, an owner without a membership, or anyone
in an inbox kept read-only, gets 403 `inbox_read_only`). A proposed rule does
not act yet: confirming it and turning it down (`POST …/rules/{rule_id}/decline`,
`conversations.decline_rule`) SHALL take the same `write` on the inbox, and
confirming SHALL make the confirming member its author, as a yes again does
(the stored author is SQL's to choose and is not shown, so the rule acts
with the rights of the member whose yes signs it). Updating, removing and restoring an active rule SHALL take its author
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

#### Scenario: A yes makes it theirs
- **WHEN** Sam proposes a rule in info@ and Leo, a sender there, says yes to it
- **THEN** it is active with Leo as its author, Leo may change it, and Ana, another sender, gets 403 `rule_not_yours` removing it

#### Scenario: Leo's rule in a shared inbox
- **WHEN** Leo taught a rule in info@ and Ana, a sender there, tries to remove it
- **THEN** the response is 403 `rule_not_yours`, and Ana's manager colleague can remove it

#### Scenario: The author leaves
- **WHEN** Leo is removed from info@
- **THEN** his rules there are paused and stop acting on new mail

#### Scenario: The author is suspended, then the suspension is lifted
- **WHEN** an administrator suspends Leo, who taught a rule in info@, and later lifts the suspension
- **THEN** while suspended the rule reads as paused with `paused_reason` `author_unavailable` and leaves new mail alone, and once lifted it is active and acts on new mail again

#### Scenario: A run going when the author is suspended
- **WHEN** Leo's rule is being applied to existing mail and Leo is suspended before its next batch
- **THEN** that batch cancels the run and changes nothing

#### Scenario: An app
- **WHEN** a client token with `conversations:write`, registered by Sam, creates a rule
- **THEN** the response is 403 with code `rule_needs_member`

#### Scenario: Kept read-only
- **WHEN** the connection feeding an inbox is removed with its mail kept while a rule's run is going
- **THEN** the run is cancelled, the rule reads as paused, and creating a rule there answers 403 `inbox_read_only`

### Requirement: Suspended authors
The module SHALL record, per workspace, whether each member is suspended,
from `workspaces.member_suspended` and `workspaces.member_unsuspended`
(`conversations.pause_suspended_author`,
`conversations.resume_unsuspended_author`), keeping for each member the
newest change: the one with the higher `seq`; a change without `seq` is
older than one with it, and two without are ordered by `at`. A duplicate, or
a change older than the one recorded, SHALL change nothing. Whenever it
records a change, the module SHALL publish `conversations.rule_changed` for
each of the member's rules that is not removed. A member leaving the
workspace SHALL be recorded as not suspended as of their departure (they
come back only by signing in) unless the change recorded carries a `seq`,
which SHALL be kept: joining again announces the newest change.
`RuleView` SHALL carry `paused_reason` when its state is `paused`:
`inbox_read_only`, else `domain_too_broad` (a `sender_domain` naming a public
suffix), else `author_cannot_write`, else `paused_by_member`, else
`author_unavailable`; and, for a proposed rule, `paused_once_kept`: the
reason it would be paused once confirmed, which is never its author's
(confirming makes the confirming member its author): only an inbox kept
read-only or a domain too broad. A
suspension is an administrator's action and SHALL not be disclosed to other
members, clients or the assistant: no rule reason, event or text readable in
the workspace names it.

#### Scenario: Delivered out of order
- **WHEN** the lift (higher `seq`) is delivered before the suspension it lifts
- **THEN** the member is recorded as not suspended and their rules stay active

#### Scenario: The clock stepped back
- **WHEN** a suspension with `seq` 1 is followed by its lift with `seq` 2 whose `at` is five minutes earlier, and both are delivered again
- **THEN** the rule is active, and `rule_changed` was published once for each change

#### Scenario: Before the counter
- **WHEN** a change without `seq` is recorded, then a counted lift with an older `at`, then a change without `seq` with a newer `at`
- **THEN** the counted lift wins and the last change is ignored

#### Scenario: Delivered twice
- **WHEN** the same suspension is delivered twice
- **THEN** the member is suspended once and `rule_changed` is published once per rule

#### Scenario: Removed while suspended, invited back after
- **WHEN** a suspended member is removed from the workspace, the suspension is lifted (which no longer reaches that workspace), and they are invited back to the inbox
- **THEN** their rules act again

#### Scenario: Back while still suspended
- **WHEN** a member whose counted suspension is recorded leaves, comes back to the inbox while still suspended, and is then lifted
- **THEN** their rule stays paused with `author_unavailable` until the lift, then acts

#### Scenario: Keeping a proposal whose author is away
- **WHEN** a member reads a proposed rule whose author is suspended, then confirms it
- **THEN** the proposal carries no `paused_reason` and no `paused_once_kept`, and once confirmed it is the confirming member's and active

### Requirement: Sending with undo
`conversations.send_draft` SHALL name the draft version it sends and SHALL
answer 409 `draft_changed` when that is not the saved one, so what is sent
is what the member last saw saved; the client SHALL send only once its last
save of what is on screen succeeded. It SHALL refuse a draft with no recipient (400
`no_recipients`), a malformed address (400 `invalid_address`) or more than
25 MiB of attachments (400 `attachments_too_large`); otherwise it SHALL schedule the
draft and answer 202 with the time it will be released, after
`CAMPBOOKS_CONVERSATIONS__UNDO_SEND_SECONDS` (default 10). Until then
`conversations.cancel_send` SHALL put it back to an editable draft; after
release it SHALL answer 409 `too_late`. A scheduled or sending draft SHALL
not be edited, deleted or sent again (409 `draft_not_editable`): a scheduled
send is taken back only by `cancel_send`, the deliberate cancel, after which
the draft may be discarded.

#### Scenario: Undo within the window
- **WHEN** Sam sends a reply and cancels it before the window ends
- **THEN** the draft is editable again and nothing is ever published for that send

#### Scenario: Sending again after an undo
- **WHEN** Sam cancels a send, edits the draft and sends it again
- **THEN** only the second send is released, once

#### Scenario: Discarding a scheduled send
- **WHEN** Sam's reply is scheduled and a request deletes its draft
- **THEN** the answer is 409 `draft_not_editable` and it stays scheduled; after `cancel_send` the delete succeeds and nothing is ever sent
