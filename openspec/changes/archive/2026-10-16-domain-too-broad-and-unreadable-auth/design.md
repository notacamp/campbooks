## Decisions

- **Re-check at match time, not by migration.** `Condition::holds` for
  `SenderDomain` returns false unless `is_owned_domain` holds. The same
  predicate gates new rules, so the two cannot drift. No stored data changes
  and no rule is deleted.
- **Surface it through the existing paused mechanism.** `Stored::effective_state`
  already derives `paused` from the author and inbox; a rule with a too broad
  domain joins it, and `paused_reason` gains `domain_too_broad`. It ranks above
  `paused_by_member` and `author_unavailable` because resuming or a lifted
  suspension would not make the rule act, and below the inbox and author
  reasons that are the more fundamental. Only removing the rule clears it.
- **Unreadable means failed.** `authentication_failed` treats `parse` returning
  `None` like the oversized case. The risk is genuine mail whose header the
  parser rejects; a test parses every provider header the repo records
  (the recorded dependabot batch and the shapes the connectors model on Gmail
  and Zoho). A header of only `none` after the authserv-id (RFC 8601) is
  rejected by `parse`; it already counted as failed when it read as no results,
  so the outcome is unchanged. Zoho's authserv-id remains unverified (#132).
