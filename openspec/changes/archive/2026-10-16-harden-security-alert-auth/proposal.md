## Why
The security-alert rule (authenticated list mail from a code host or package
registry becomes a sure to-do) shipped with known gaps, found in review:
a DKIM signature that DMARC then refused still authenticated the mail; the
registrable-domain guess knew 24 public suffixes; a person named "GitHub …"
could pass for the platform; a decide pass beside a provider spam move could
store an open need after the move; and two paths had no test.

## What Changes
- A `dkim=pass` authenticates only when no `dmarc` result in the same header
  says anything but `pass`.
- An `Authentication-Results` value over 8 KiB reads as no results.
- The registrable domain, and "a public suffix", come from the Public Suffix
  List (the `psl` crate, compiled in, ICANN and private sections), replacing
  the 24-entry list. A bare public suffix as From authenticates nothing.
- On a listed platform the sender's whole display name must be the
  platform's name; a name that merely holds it as a word no longer counts.
- A need refresh takes the message it reads `FOR SHARE` before the needs row,
  so it cannot store an open need for a message a concurrent provider move to
  spam or the trash is committing.
- Tests: leaving spam re-raises the need; a trash round trip keeps a member's
  confirmation and a Done.

## Capabilities
- Modified: `conversations`.

## Impact
Crates `campbooks-ai` (new dependency `psl`, MIT OR Apache-2.0) and
`campbooks-conversations`. No migration, no event, no operation. The
`X-GitHub-Reason` veto is not built: the connectors keep no such header
(see design).
Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/133
