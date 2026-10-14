## ADDED Requirements

### Requirement: Scout's memory lists the rules
Settings SHALL have a Scout's memory page (`/w/{id}/settings/memory`) that
lists, per inbox the user can see, its rules from `conversations.list_rules`
as one sentence each, built from the rule's conditions and actions in the
user's language ("Mail from notifications@github.com about "pull request" →
Updates · 1,000 so far"), with where it came from (an action, an onboarding
answer, the user's own words quoted, corrections), who taught it, when it last
acted and how many times the user changed one back. Rules in a shared inbox
SHALL say they apply for everyone. On phones the page SHALL be one column with
no horizontal scrolling at 375px.

#### Scenario: Two inboxes
- **WHEN** Ana opens Scout's memory with rules in her own inbox and in info@
- **THEN** she sees them under each inbox's address, the info@ ones marked "For everyone"

### Requirement: Rules waiting for a yes
Proposed rules SHALL be listed first, under "Waiting for you", each with why
it was proposed and its preview count, Keep this rule (Y) calling
`conversations.confirm_rule` and No (Esc) calling `conversations.remove_rule`.

#### Scenario: Learned from corrections
- **WHEN** a rule was proposed because Ana marked four bills from billing@hosting.example Not needed
- **THEN** it waits under "Waiting for you" and does nothing until she keeps it

### Requirement: Editing and removing a rule
Edit SHALL let the user change the likeness (among the same choices the offer
had) and the actions, with the preview count, through
`conversations.update_rule`. Remove SHALL confirm in place, offering to put
back what the rule changed, and call `conversations.remove_rule`. Edit and
Remove SHALL be disabled, with the reason, on a rule the user may not change.

#### Scenario: Remove and put back
- **WHEN** Ana removes the studio-news label rule with "Also take the label off the 41 it labelled"
- **THEN** the rule is gone and the 41 conversations lose the label
