## Why

Two non-blocking findings from the review of #501 (campbooks-cloud#163):

- A taught rule stored before the full Public Suffix List was used, with a
  `sender_domain` that is now known to be a public suffix (`github.io`), still
  matches every sender under it. New rules cannot name one; old ones are never
  re-checked.
- A present `Authentication-Results` header that cannot be parsed (an
  unbalanced quote or comment a sender echoed into it) says nothing, so a
  `dmarc=fail` in it is erased.

## What changes

- A `sender_domain` condition whose value is not an owned domain never matches
  (fail closed). Such a rule reads as paused with the new `paused_reason`
  `domain_too_broad`; the member can still remove it.
- A present but unreadable `Authentication-Results` counts as authentication
  failed. Absent still says nothing. Hint only: it never promotes a decision.
- Every recorded provider header in the repo's fixtures is asserted to parse,
  so genuine mail is not flagged.

## Impact

`campbooks-ai` (taught matching, facts), conversations (`PausedReason`,
`RuleView`, OpenAPI), the rules list copy in four languages. No migration;
production holds no taught rules.
