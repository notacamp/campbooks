# Design

Prototype (v6, desktop and 375px): https://claude.ai/artifact/FyiDtRhSG6zr6xfwemYpsU,
its "Scout noticed" card, the offers after Report and delete and after
Already paid, and the smart-rule confirmation. The rules engine and its owner
decisions are `archive/2026-10-14-taught-rules/design.md`; Report and delete
is `archive/2026-10-17-trash-and-spam`.

## Data ownership and events

Everything is in schema `conversations`, forced row-level security, every
row with its inbox:

- `rule_notices` (`id`, `inbox_id`, `kind` `keep_off_home|archive_on_arrival`,
  `sender_address`, `sender_name`, `conditions`, `actions`, `evidence`
  (counts), `state` `open|accepted`, `rule_id`, `accepted_at`,
  `refreshed_at`), unique per inbox, kind and sender.
- `rule_notice_answers` (`inbox_id`, `user_id`, `kind`, `sender_address`,
  `answer` `not_now|never`, `until`): each member's own.
- `rule_notice_scans` (`inbox_id`, `scanned_at`).

Inbox purges cascade; an inbox kept read-only forgets them
(`forget_connection`); a departing member's answers go (`forget_member`).
Rules keep their shapes: the new condition and actions are new JSON values in
`rules.conditions` / `rules.actions`; `rule_applications.before` gains
`binned` (`trash|spam`) so a revert takes the mail out again. No new event:
a notice is the caller's to fetch, and rule changes publish
`conversations.rule_changed` as before.

## Decision 1: what "never see emails like this" is

| | **a. Report as spam, on mail failing its sender checks for that domain (chosen)** | b. Trash everything from the sender | c. Trash everything from the domain |
| --- | --- | --- | --- |
| A phishing mail pretending to be paypal.com | the next fake goes to spam; real PayPal mail (passing its checks) is untouched | the next fake from another address gets through | real PayPal mail goes too |
| A look-alike domain that signs its own mail | the sender (never a whole domain to the bin) | covered | covered |
| Tells the provider | yes (spam is the report) | no | no |

**Chosen: a.** "Fails its sender checks" is the receiving provider's
`Authentication-Results` read as the facts already read it
(`EmailFacts::authentication_failed`: a DMARC fail, or neither a DMARC pass
for the From domain nor an aligned DKIM pass; absent results say nothing).
The offer, and the condition itself (`sender_checks_fail`), use only a
`dmarc=fail` under a published `reject` or `quarantine` policy (Gmail's
`dmarc=fail (p=REJECT …)`, read from the result's own comment): the domain
asks for such mail to be refused, so its genuine mail passes. A domain that
authenticates nothing, or publishes `p=none`, fails with genuine mail sent
through other tools too, and a policy that cannot be read (some providers
leave it out) offers nothing: the offer is then the sender. The condition
never matches mail from an address the inbox writes to, on arrival and over
existing mail. The broader test (`EmailFacts::authentication_failed`) still
keeps `mark_paid` off a bill, where broader is safer. `create_rule` refuses a
trash or spam rule with nothing narrower than a domain.
Then the offer's default is the From domain *with* that condition,
never the bare domain, and only for a rule that reports or deletes: the
offer after Already paid never offers "pretending to be" (a pretender's
bills are never marked paid, and `mark_paid` never acts on mail failing its
checks at all). A whole domain is never offered to the bin: a colleague of
the phisher's victim may write from it. The action is `spam` (Report and delete reports to the
provider); `trash` exists for rules that just delete, without a report. The
brand a phishing mail imitates in its text is not read (no content rules,
ADR 0018).

**The suspicious guard.** Rules never hide suspicious mail (Decision 3 of
taught rules): no archive, read, move out of Primary, "needs nothing" or
`surface: never`. A trash or spam rule is the exception: it exists for that
mail. "Always show" still outranks it, and its applications are undone like
any other (untrash puts each message back where it was; not spam puts
received mail in the inbox).

## Decision 2: "its bills are paid"

| | **a. Answer the money question `paid` (chosen)** | b. `surface: never` | c. Resolve the Pay need as done on arrival |
| --- | --- | --- | --- |
| A reply the same sender asks for | still raised | hidden too | still raised |
| Where it lives | a `taught` decision, like `needs_reply` | a `taught` decision | a write per arrival, no decision |
| Explainable in "How Campbooks decided" | yes, "A rule" | yes | no |

**Chosen: a.** The rule answers `money: paid` (acted, source `taught`); the
Pay rule already gives an acted non-`owed` money answer the last word, so no
"Is it paid?" is raised and an open one clears with cause `rule`. Nothing is
filed (no documents module yet).

## Decision 3: how Scout notices

| Signal | Read from | Used for |
| --- | --- | --- |
| How often | conversations whose newest inbound message is the sender's, in the window | at least `min_messages` (8) |
| How regularly | their sent dates: distinct days (`min_days`, 4), median gap | not a burst; "daily / weekly / monthly" in the sentence |
| What it is | the stream the decide pass sorted most of it to; `money = owed`; `suspicious` | the stream to file it under; never a bill or suspicious mail |
| Whether it matters | an open Reply/Do need, or one the member marked Done or confirmed; a reply from the inbox; a person (not list, automatic or no-reply mail); a correspondent | never offered |
| What the member does | read (opened here or at the provider); archived or in the trash while unread | at most 25% opened → keep it off Home and file it; at least 70% archived unread → archive it as it arrives |

No subject, body, attachment name or phrase list is read. Every threshold is
a setting (`CAMPBOOKS_CONVERSATIONS__NOTICES__*`) and a prior: none was
measured, and the owner's labelled mail should tune them (an eval needs the
owner to say, per suggestion, whether it was right).

Kinds today: `keep_off_home` (move to the stream + `surface: never`) and
`archive_on_arrival` (archive + `surface: never`). Recurring invoices
(remind before the debit) wait for invoice reading (#170).

**When it runs.** A scheduled job cannot read workspace tables, so the pass is
started by `list_rule_notices` for an inbox not looked at within
`every_hours` (24): one job per inbox (unique key), behind live work. What it
finds shows the next time Home asks. A sender a rule of the inbox already
names is skipped, and so is one whose rule would change nothing.

## Decision 4: one confirmation for every rule

| | **a. One dialog in the conversations client module, used by every offer (chosen)** | b. Each offer saves at once with Undo (teach once today) | c. Confirm in Scout's memory only |
| --- | --- | --- | --- |
| "Nothing saved before Save smart rule" | yes | no | yes, far from the moment |
| Reuse | one component, the existing preview | n/a | n/a |

**Chosen: a.** `SmartRuleConfirm` (exported by the conversations client
module) shows the rule as a sentence labelled "Smart rule", where it lives,
what it matches (`conversations.preview_rule`: a count and up to three
subjects as plain text; suspicious mail is counted, never shown), "Also apply
it to the N already here" (on), Save smart rule (⌘↵) and Cancel (Esc), then
its result for about a second. Teach once's accept opens it too (its run's
progress and Undo all follow as before). Teach Scout's composer (#121) keeps
its own inline form, which already asks the same before saving; it shows the
same "Smart rule" card.

## Scopes and operations

| operationId | Method, path (under `/api/v1/w/{workspace_id}`) | Scope | Who |
| --- | --- | --- | --- |
| `conversations.suggest_rules` | GET `/conversations/{id}/rule-suggestions?action=report\|paid` (new values) | `conversations:read` | `write` on the inbox |
| `conversations.list_rule_notices` | GET `/rule-notices` | `conversations:read` | the inboxes the caller may write in |
| `conversations.answer_rule_notice` | POST `/rule-notices/{id}/answer` (undo `clear_rule_notice_answer`) | `conversations:write` | `write` on the inbox, a member |
| `conversations.clear_rule_notice_answer` | DELETE `/rule-notices/{id}/answer` | `conversations:write` | the same |
| `conversations.create_rule` | `notice_id` (new) | `conversations:write` | as before |

A suggestion of another inbox, or one already accepted, is refused
(`invalid_rule`); one the caller cannot see is 404. A rule saved from a
suggestion has origin `suggestion` and acts at once: the member confirmed it
(the "rules from evidence ask first" rule is for rules Scout makes on its
own). Removing it with `restore` within the restore window (Undo) offers the
suggestion again.

## AI use

None. The engine, the offers and the pass are deterministic; no role is
called, and nothing changes when no AI is configured. Every rule is the
member's, confirmed, and undoable per conversation and as a whole.

## Does this copy the Rails app?

No: the Rails app had no offer after an action and no noticed suggestions.
