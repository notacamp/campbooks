## Why
The authentication facts the decide pass hands a model (and the sender
alignment that taught rules and Reply-To checks use) had known gaps, found
in the security-alert review (#487): a DKIM signature that DMARC then refused
still authenticated the mail; the registrable-domain guess knew 24 public
suffixes; the results header had no length bound; and a decide pass beside a
provider spam move could store an open need after the move; two need paths
had no test.

## What Changes
- A `dkim=pass` authenticates only when no `dmarc` result in the same header
  refuses it (`fail`, `quarantine`, `reject`, `policy`).
- An `Authentication-Results` value over 8 KiB is unread and counts as
  authentication failed (fail closed).
- The registrable domain, and "a public suffix", come from the Public Suffix
  List (the `psl` crate, compiled in, ICANN and private sections), replacing
  the 24-entry list. A bare public suffix as From authenticates nothing.
- A conversation's first need is not stored for a message a concurrent
  provider move to spam or the trash has locked (`FOR SHARE SKIP LOCKED`; it
  never waits).
- Tests: leaving spam re-raises the need; a trash round trip keeps a member's
  confirmation and a Done.

## Capabilities
- Modified: `ai-decide-facts`, `conversations`.

## Impact
Crates `campbooks-ai` (new dependency `psl`, MIT OR Apache-2.0) and
`campbooks-conversations`. No migration, no event, no operation.
The review's other two items (the platform display-name rule and the
`X-GitHub-Reason` veto) concerned the `signals.security_alert` rule, which
`next` removed in the meantime (facts only say no, ADR 0018); nothing is left
to harden there (see design).
Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/133
