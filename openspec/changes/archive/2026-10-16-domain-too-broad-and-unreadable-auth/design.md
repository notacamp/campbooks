## Decisions

- **Re-check at match time, not by migration.** `Condition::holds` for
  `SenderDomain` returns false unless `is_owned_domain` holds. The same
  predicate gates new rules, so the two cannot drift. No stored data changes
  and no rule is deleted.
- **Surface it through the existing paused mechanism.** `Stored::effective_state`
  already derives `paused` from the author and inbox; a rule with a too broad
  domain joins it, and `paused_reason` gains `domain_too_broad`. It ranks right after
  `inbox_read_only` and above the author's reasons, because adopting the rule,
  resuming it or a lifted suspension would not make it act. Only removing the rule clears it.
- **Unreadable means failed.** `authentication_failed` treats `parse` returning
  `None` like the oversized case. The risk is genuine mail whose header the
  parser rejects; a test parses every provider header the repo records
  (the recorded dependabot batch and the shapes the connectors model on Gmail
  and Zoho, with more Gmail shapes: ARC, `dara`, an empty `smtp.mailfrom=`,
  two DKIM signers). A section that is exactly `none` after the authserv-id
  (RFC 8601: the receiver checked nothing) parses as no results and says
  nothing, as before; any other empty or malformed shape (`mx;` alone, `none`
  beside another result) fails closed. `parse` also reads the whitespace the
  RFC allows around `=` and `/` and a comment before `=`, so such headers are
  not called unreadable. Zoho's authserv-id remains unverified (#132).
- **Confirm re-validates.** A proposal stored before its domain became a public
  suffix answers 400 `invalid_rule` on Keep, like creating one, so it is not kept.
