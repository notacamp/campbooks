## Why

The owner's Home prototype (campbooks-cloud#164, v6) offers smart rules at the
right moment: after "Report and delete" ("Never see emails like this
again?"), after "Already paid" ("Mark EDP's invoices paid from now on"), and
before the member asks, when Scout notices mail they get a lot of. Every rule,
whatever offered it, is confirmed as a smart rule before it is saved. The
owner: preferences become deterministic smart rules, never prompt text, and
nothing parses email content.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/169.
Builds on taught rules (campbooks-cloud#125) and on `trash-and-spam`
(campbooks#540, Report and delete's operation).

Modules touched:

- **`campbooks-ai`**: the taught-rule engine gains the condition "fails its
  sender checks" (a structured fact: the receiving provider's
  authentication results) and the actions trash, spam and "its bills are
  paid".
- **`conversations`**: the new actions on arrival and over existing mail
  (with undo); offers after Report and Already paid
  (`conversations.suggest_rules`); "Scout noticed": a pass per inbox,
  stored suggestions, `conversations.list_rule_notices`,
  `conversations.answer_rule_notice`, `conversations.clear_rule_notice_answer`,
  and `create_rule`'s `notice_id`.
- **Client** (`conversations`, `attention`, `assistant`): the smart-rule
  confirmation dialog every rule goes through, the offers on Home, "Report
  and delete" on a warning, "Scout noticed" at the bottom of Home.

No new module and no new cross-module dependency.

## What Changes

- **Smart rules for the trash, spam and paid bills.** A rule may report mail
  as spam or put it in the trash; such a rule acts on suspicious mail too (it
  is about that mail), but never against "always show". A rule may say a
  sender's bills are paid: the money question is answered `paid` for its
  mail, so "Is it paid?" is never asked (and an open one clears). A
  condition "fails its sender checks" reads the receiving provider's
  authentication results, never the text.
- **Offers after an action.** After Report and delete, `suggest_rules`
  (`action=report`) offers to report mail like it: when the provider says the
  sender explicitly failed DMARC, "mail that says it is from paypal.com but
  fails its sender checks" first; else the sender; never a whole domain.
  After Already paid (`action=paid`): the sender's bills, or its
  domain's. Both are offered even with nothing like it yet (they are about
  mail still to come); a person the inbox writes to is never offered a rule
  that hides their mail (paid is not hiding).
- **Scout noticed.** A pass per inbox (at most daily, started when Home asks)
  counts each sender's recent mail: how often and how regularly it comes, the
  stream the decide pass sorted it to, whether it ever asked something,
  looked suspicious or asks for a payment, and what the inbox did with it
  (opened, archived or deleted unread, replied). Mail rarely opened is
  offered "keep it off Home and file it under Updates"; mail archived unread
  "archive it as it arrives". At most a few per inbox, one sender and kind
  each, dropped once the sender no longer qualifies. "Not now" hides one for
  two weeks, "Don't suggest this" for good, per member. Thresholds are
  settings, and priors until the owner's mail tunes them.
- **One confirmation for every rule.** Saying yes to any offer (teach once
  included, which saved at once before) opens the same dialog: the rule as a
  sentence labelled "Smart rule", that it runs on its own with no AI and
  lives in Scout's memory → Smart rules where it can be changed or turned
  off, what it matches (a count and a few subjects as plain text), "Also
  apply it to the N already here" (on), Save smart rule (⌘↵) / Cancel (Esc).
  Then "Smart rule saved · it's in Scout's memory" for a second, and the
  toast's Undo removes it and puts back what it changed.

## Non-goals

- Recurring invoices: "remind me before the debit" and filing in Paper wait
  for invoice reading (#170) and documents (#45).
- Reading the impersonated brand from the email (the model would have to
  carry it as a fact; it does not).
- Editing a suggestion's rule before saving (Scout's memory's builder does
  that after).
- Deleting mail for good.
