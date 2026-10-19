## Why

Home's Forward picker (#166) listed every visible sender, including the
phishing sender under review and no-reply addresses: forwarding mail to a
suspected impersonator, or to an address nobody reads, is almost never meant
(campbooks-cloud#233). The review of #548 also found three small Forward
dialog slips: Esc while a forward started left the drafts cache stale, so a
reopen could start a second pristine draft; a drafts error during a start
still opened the composer after the error was shown; and the "no file" toast
always blamed pictures in the text.

## What Changes

- `conversations.list_senders` takes `reachable`: only senders it is safe to
  write to. It leaves out no-reply addresses, senders whose newest mail
  fails their checks or pretends to be them, and senders with mail under an
  open review. Facts only (ADR 0018): addresses, the provider's
  authentication results and review needs, never what the mail says.
  Without it, the answer is as before (Scout's memory and questions).
- Forward's picker asks with `reachable`, so a name typed for a left-out
  sender finds nobody (no warning, no list).
- Forward always records the draft it made, also when closed or given up on
  meanwhile, so a reopen addresses it again; and it opens the composer only
  for the start still current, never after it said the start failed.
- Open attachment with no file says "This email has no file to open".

## Impact

- Specs: conversations (Who writes to the caller), home-ui (Scout's ranked
  actions).
- Code: `list_senders` (runtime query, its offline cache entry removed),
  OpenAPI, `useSenders`, `ForwardDialog`, attention i18n.
