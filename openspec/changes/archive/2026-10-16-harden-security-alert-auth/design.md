## Context
Follow-ups to the security-alert review (#487), applied to the authentication
facts in `signals/facts.rs`. Facts only say no; nothing here reads message
content to decide.

## Decisions
- **DKIM defers to a refusing DMARC.** An aligned `dkim=pass` is the
  sender's own signature, but a DMARC result that refuses the mail (`fail`,
  `quarantine`, `reject`, `policy`: the domain's published policy said no)
  turns the DKIM path off. `none` means no policy is published (RFC 7489
  §11.2), `bestguesspass` is Microsoft's guess, and `temperror`/`permerror`
  are no verdict: none refuses, and our audience (small businesses) often
  signs with DKIM and publishes no DMARC, so they keep the DKIM path, as does
  no `dmarc` result at all (Zoho adds none). The DMARC path is unchanged
  (exactly one `pass` result whose `header.from` is the From domain). The
  result feeds the `authentication_failed` hint.
- **Length cap, fail closed.** `auth_results::MAX_LEN` is 8 KiB. A provider's
  header is a few hundred bytes; the parts a sender can pad (envelope
  sender, comments) would otherwise make the tokeniser do unbounded work.
  Over the cap `parse` returns `None`, and `Facts::authentication_failed`
  treats a present oversized header as a failure (not as absent), so padding
  a spoofed `dmarc=fail` past the cap cannot drop the warning. Absent or
  malformed headers still say nothing.
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
- **Spam race and lock order.** A decide pass reads a message, a provider
  move to spam commits, and the pass then stores an open need. The race
  exists only when the conversation has no needs row yet: with one,
  `current` locks it `FOR UPDATE`, which serialises refreshes, and the
  move's own refresh (the end of `apply_changes`) clears a stale need. So
  only a first insert re-checks: it reads the need's message
  `FOR SHARE SKIP LOCKED` and stores nothing when the row is locked by
  another change (that change's own refresh decides) or is in spam or the
  trash. It never waits on a message lock, so it adds no wait edge. The
  callers of `needs::refresh` take: `apply_changes`, messages, then the
  conversation's counts, then the needs row; `judge` and `reopen`, the
  conversation then the needs row (a row always exists there, so they never
  reach the check); rules and flags, the conversation row; the decide pass
  and the backfill, the needs row. An earlier draft share-locked the
  messages before the needs row for every refresh; that put messages after
  the needs row for `judge`, against `apply_changes`, and could deadlock, so
  it was dropped. The share lock taken by a first insert is held to the end
  of the transaction (the backfill's page), which holds a later move back
  but is never waited on by the refresh. A skipped conversation is not
  retried by the backfill; the change that held the lock settles it.
- **What a trash round trip does.** A message leaving the trash or spam
  raises its need again through the ordinary path (newest eligible inbound
  message, its answers). A Done or Not needed verdict stays: the need clears
  as `done` / `not_needed`, trash or no trash. A confirmation stays too: the
  need reopens with basis `member`.

## Risks
- A stale `psl` version misses new suffixes; the dependency is updated like
  any other (dependabot).
