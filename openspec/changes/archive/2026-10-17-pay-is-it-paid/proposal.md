# Proposal

Modules: `conversations` (needs, the model pass, the needs backfill, the
labelling export), `campbooks-ai` (`decide`: the triage questions, the
answers check), `server` (the `labelling export` command), `xtask` (the
decide eval), `clients/app` (Home's Pay rows, once campbooks#515 lands).
Tracking: https://github.com/notacamp/campbooks-cloud/issues/167 (part of
campbooks-cloud#164). No new module and no cross-module dependency.

## Why

Pay is raised whenever an accepted `asks_action` meets a message sorted to
Receipts. Jev reads many receipts and payment confirmations as asking
something, so invoices already paid and "payment received" mail land in Pay:
on the owner's first day with Jev, 11 of 23 Receipts conversations became
"Pay". Campbooks also cannot see the bank, so even a real bill may be paid
already.

## What Changes

- **A money question.** The triage questions gain a `choice`, `money`:
  `owed` (asks for a payment still owed), `paid` (confirms a payment already
  made) or `neither`, asked in the same decide call. The answers check gains a
  bill and a payment confirmation, and money expectations on six of its
  cases (50 expected answers, each money option covered).
- **Pay only for a payment still owed, as a question.** A Pay need is raised
  only when the money answer is `owed` (accepted by the policy, or the
  model's at 0.8 or more), never for a receipt or a confirmation, and never
  for list mail. Mail in Receipts needs nothing else. A Pay need arrives as a
  question, "Is it paid?" (trust `check`, question `money`).
- **Already paid / Still to pay.** `conversations.resolve_need` takes
  `already_paid` (cleared as done; nothing filed yet) and `still_to_pay` (a
  sure Pay need, a correction confirming `owed`), only for a Pay need (409
  `not_a_payment` otherwise). Not needed on a Pay need stores a correction of
  `money` from `owed` to `not_owed`. Due dates stay null until campbooks-cloud#146.
- **The policy records `money`** until the owner's labelled mail gives it an
  operating point: nothing in the fictional eval set is labelled for it, and
  agent-written labels are not used for thresholds.
- **Owner labelling.** `campbooks-server labelling export --workspace <id>
  --days 90 --out <folder>` writes the workspace's recent money mail as an
  unlabelled eval set and a `labels.csv` sheet; `cargo xtask eval decide
  --set-dir <folder>` reads the owner's labels from it. The folder is real
  mail, kept on the owner's machine, never committed.
- **Upgrade.** A migration widens the needs checks and queues every inbox
  for the needs backfill: each recent need is worked out again under the new
  rule, and the model is asked again about each recent newest inbound message
  that lacks a checked answer to any triage question (so it answers `money`).
- **Home (after campbooks#515).** Pay rows ask "<Payee> sent an invoice. Is
  it paid?" with Already paid (1) and Still to pay (2), "All N already paid"
  on the Pay group with one Undo; Still to pay turns the row into "Pay
  <payee>" with its due date when known and a reminder as first action.

## Non-goals

- Reading amounts or due dates from the invoice (campbooks-cloud#146,
  campbooks-cloud#170). No amount is parsed from text.
- Filing a paid invoice or offering a smart rule after Already paid
  (campbooks-cloud#169); the client leaves a hook for it.
- Bank matching (Money module).
- An operating point for `money` from fictional mail.
