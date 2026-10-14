## Why

The backend of teach once is in (`taught-rules`, notacamp/campbooks#482): a
member's action on one email can become a deterministic rule that does the
same to the mail that looks like it, now and as new mail arrives. Nothing in
the app offers it yet. The owner (2026-10-14): "the purpose of actions of
email is that visor does them for me, at least after the first."

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/125.
Prototype (desktop and 375px): https://claude.ai/artifact/2y7z5yvKsMpKC6nVTsPrw7.
Design: `openspec/changes/archive/2026-10-14-taught-rules/design.md`.

Modules touched: the client only (`clients/app`): `lib/ui` gains `Offer` and
`Progress`; the `conversations` client module gains the offer, the run's
progress with Undo all, the rule line on rows and in the reader, the
exception question, and `RulesList` for Scout's memory. No server change.

## What Changes

- **The offer.** After the member moves (M), archives (E), marks read (U) or
  marks Not needed (X) one conversation, the inbox asks
  `conversations.suggest_rules` and, when there is a suggestion, docks an
  offer above the toasts (across the bottom on phones): "Move all 1,000 like
  this to Updates, and new ones too?", the likeness choices with their
  counts, three sample subjects, whether any are left alone as suspicious,
  "For everyone in info@" in a shared inbox, and two actions: accept (1) and
  "Just this one" (Esc, which turns the suggestion down for 30 days).
- **The run.** Accepting creates the rule and turns the offer into the run's
  progress from `conversations.get_rule` (live through the change feed):
  "Finding all the mail like this", then "Moving 1,000 to Updates · 340 of
  1,000 done", then "Moved 998 to Updates. New ones will go there too.", with
  Undo all. The undo stack (Z) also undoes the whole rule.
- **Visible after.** A row a rule handled says "Campbooks moved this · like
  1,000 others from notifications@github.com" in the slot suggestions use; the
  reader's note says the same with "Stop doing this" (Undo makes the rule
  again).
- **Exceptions.** When the member changes back what a rule did for the
  second time in 30 days, the offer asks "You changed back 2 this month. Keep
  the rule …?" with Remove the rule (1) and Keep it (Esc).
- **Rules list.** `RulesList` (exported from the conversations module) lists
  the rules as sentences, proposed ones first with Keep this rule and No, then
  each inbox's with where they came from, how often they acted, "For
  everyone" in a shared inbox, and Remove (asking in place whether to put the
  mail back). It fills the rules slot of the Scout's memory page that #121
  builds; this change adds no route.

## Non-goals

- The Scout's memory page itself, its composer and manual builder (#121).
- Editing a rule's likeness or actions (the page's builder, #121).
- Offering from Home's actions, labels (no label UI yet) or the search page.
- The narrower likeness in the exception question (follow-up: the server
  knows the likenesses; asking needs a suggestion for an exception).
