# Proposal

## Why

Follow-ups from the review and hand test of "Always show images from a
sender you trust" (#549):
- The free mail list had gaps (qq.com, naver.com, yahoo.co.jp, ISP mail…), so
  "anyone at" a provider could be trusted, and any account there that passed
  the provider's DKIM would get images loaded.
- The client compared the exact domain while the server compared the
  registrable one, so `mail.yahoo.fr` was offered and then refused.
- The offer showed where it could never take effect (already trusted, failing
  checks, a provider without checks), ⇧A could open it on mail with no remote
  images, a shared inbox read "you trust", and the copy said no AI was
  involved although images wait for Scout's check.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/235

## What Changes

- `campbooks-ai`: the free mail list grows to 342 providers and internet
  providers' mail, compared by registrable domain; it is the one list, and
  the web client's copy is checked against it in a test.
- `conversations`: the message body says whether trusting its sender can take
  effect (`images_offer`), the domain that may be offered (`images_domain`:
  owned, never a free mail provider's) and why images stay held back
  (`images_held`: `under_review`, `checks_fail`, `cannot_authenticate`,
  `not_checked`). Creating a rule for a sender an image rule already covers
  is refused (409 `images_already_trusted`).
- Reader: the offer and ⇧A follow `images_offer` (nothing without remote
  images); the domain choice follows `images_domain`; a one-line note says
  why images stay held back; a shared inbox reads "this inbox trusts X". The
  confirmation says images wait for Scout's check.
- CHANGELOG corrected: the rule has no AI, but images wait for Scout's check.

## Non-goals

- No change to which mail is trusted: the rule, its checks and the sanitiser
  are as #549 left them.
