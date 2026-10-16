## Context
Follow-ups to the security-alert review (#487), applied to the authentication
facts in `signals/facts.rs`. Facts only say no; nothing here reads message
content to decide.

## Decisions
- **DKIM defers to DMARC.** An aligned `dkim=pass` is the sender's own
  signature, but DMARC is the verdict on alignment and policy. Any `dmarc`
  result other than `pass` (fail, none, quarantine, temperror, permerror,
  policy…) in the header turns the DKIM path off. No `dmarc` result at all
  (Zoho adds none) keeps the DKIM path. The DMARC path is unchanged (exactly
  one `pass` result whose `header.from` is the From domain). The result feeds
  the `authentication_failed` hint, so a refused signature now reads as a
  failure to the model.
- **Length cap.** `auth_results::MAX_LEN` is 8 KiB. A provider's header is a
  few hundred bytes; the parts a sender can pad (envelope sender, comments)
  would otherwise make the tokeniser do unbounded work. Over the cap reads as
  no results, so nothing authenticates (fail closed).
- **Public Suffix List, both sections.** `psl::domain_str` is compiled in:
  no fetch, no network at runtime; the list is as old as the pinned crate.
  ICANN-only would call `a.github.io` and `b.github.io` the same
  organisation, and `x.blogspot.com` and `y.blogspot.com` too; anyone can
  publish under those, so a signature from `a.github.io` would "align" with
  mail from `b.github.io`, and a taught `sender_domain` of `github.io` would
  match everyone. Both sections count. The cost is that a tenant under a
  private suffix is its own organisation, which is right for alignment.
  Fallbacks stay conservative: a host the list cannot split (a bare suffix,
  an empty label) is returned whole; a TLD the list does not know is its own
  suffix (the list's default rule), giving two labels as before; internationalised
  domains are refused by `plain_domain` before alignment. A bare public
  suffix as the From domain authenticates nothing. The RFC 2606
  `example.com/org/net` suffixes keep their three labels, so fixtures stand
  for many organisations. `PUBLIC_SUFFIXES` is removed (no other user).
  `psl` is MIT OR Apache-2.0, already allowed by `deny.toml`.
- **Security-alert rule gone.** While this change was in review, `next`
  removed the content rule `signals.security_alert` and its platform list
  (the decide model reads content; facts only say no). The display-name
  hardening (a person named "GitHub Smith") and the provider reason-header
  veto (`X-GitHub-Reason`) therefore have nothing to apply to and are not
  built. The connectors keep only `List-Id`, `List-Unsubscribe`, `Precedence`,
  `Auto-Submitted` and the provider's `Authentication-Results`; if a future
  facts rule needs reason headers it is a connector contract change.
- **Spam race and lock order.** Before this change a refresh read the
  newest inbound message without a lock, then stored the need. `apply_changes`
  updates a message's `in_spam`/`in_trash` in its transaction and refreshes
  the need at the end of it, so the order there is message row, then needs
  row. The refresh now locks, in one statement ordered by id and before the
  needs row, the two messages a need can come from (the newest inbound one
  outside spam and the trash, and the stored need's message) `FOR SHARE`.
  Same order, so no cycle with a move. A refresh beside an uncommitted move
  waits and then reads the committed flags; a move that starts later waits
  for the refresh, and its own refresh then clears the need. Two moves
  touching several messages of one conversation in opposite orders could
  deadlock, as before this change; Postgres aborts one and the job retries.
- **What a trash round trip does.** A message leaving the trash or spam
  raises its need again through the ordinary path (newest eligible inbound
  message, its answers). A Done or Not needed verdict stays: the need clears
  as `done` / `not_needed`, trash or no trash. A confirmation stays too: the
  need reopens with basis `member`.

## Risks
- A stale `psl` version misses new suffixes; the dependency is updated like
  any other (dependabot).
