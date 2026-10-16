# 0018 — A decision model reads email content; facts only say no

**Status:** Accepted · 2026-10-16 · amends [0005](0005-open-weight-ai-by-role.md) and [0009](0009-decide-role-typed-decision-model.md)

## Context
Triage answers six questions about every inbound message (`stream`,
`needs_reply`, `asks_action`, `has_deadline`, `urgency`, `suspicious`).
Until now only hand-written rules could act on them:
`campbooks_ai::decide::signals::email` holds about 2,400 lines of phrase
lists and regular expressions in four languages, and the policy let no
model answer act. One of those rules answered "needs a reply" for any
message whose text holds a `?`, at the confidence the policy acts on, so a
shipping notice ending "Questions? Contact our support team" became a Reply
need in production. The 94% precision the rules were credited with came
from 320 fictional emails written in the same change as the rules, tuned on
one half and scored on the other: it measured the rules against their own
author.

Email is too varied for phrase lists. Every rule that reads the text is a
guess about language that the next newsletter, notification or polite
request breaks.

Laya, the `decide` model chosen in ADR 0009, does not work without
fine-tuning on our own decisions (campbooks-cloud#128). Measured on 2026-10-16
(campbooks-cloud#144) on 60 emails of the eval set and 16 notifications and
personal questions:

| | Laya multilingual | Jev 1.13 | Jev 1.14 |
|---|---|---|---|
| `stream` accuracy | 17% | 100% | 100% |
| `needs_reply`: said yes / truly yes | 59 / 13 | 19 / 13 | 23 / 13 |
| `asks_action` accuracy | 53% | 97% | 92% |
| notifications with "?" read as needing a reply | 10 of 10 | 1 of 10 | 1 of 10 |
| latency per message | about 1.2 s (laptop CPU) | 0.4 s | 0.4 s |

Jev (TypeSafe) is a hosted decision model with the same request shape as
Laya's `/systemone` (Laya's server copies it), served through OpenRouter's
decisions endpoint. It is closed and hosted in the US.

## Decision
- **Understanding what an email says is a model's job.** No code reads a
  message's subject, body or attachment names to decide what it means: no
  phrase lists, keyword lists or regular expressions over content, in any
  language. Text hygiene (HTML to text, dropping quoted history) is not
  understanding and stays allowed.
- **Facts only say no.** Deterministic code reads structured facts only: the
  mailing-list and automation headers (`List-Id`, `List-Unsubscribe`,
  `Precedence`, `Auto-Submitted`), the sender's and recipients' addresses,
  the receiving provider's authentication results, attachment media types.
  From them it may answer "no" (a list message or a no-reply address needs
  no reply), never "yes". The facts are also told to the model as hints.
  The rules' input type holds no subject or body, so a content rule cannot
  be written against it.
- **The model's answers act** where the eval shows they hold their target
  precision, from the operating point it measured, exactly as the rules' did.
  An acted "no" from the facts still wins over the model's "yes".
- **A hosted decision model may serve `decide`** (amends ADR 0005). The
  shipped defaults stay open-weight: self-hosted instances default to Laya or
  to no `decide` role. An instance may point `decide` at a hosted decision
  model, Jev first, by configuration. Content leaves the instance only when
  the instance admin allows it (`CAMPBOOKS_AI__DECIDE__EXTERNAL_CONTENT=allow`).
- **Jev joins Laya as a `decide` provider** (amends ADR 0009): same protocol,
  a configurable endpoint path. Laya remains the self-hosted option and the
  model to fine-tune, on answers and corrections we collect.
- **Extraction stays with `complete`.** A due date or the sentence that asks
  is extracted, not decided; until that extraction exists a need carries no
  due date.

## Consequences
- The rules shrink to a small facts module, and the reasons shown under
  "How Campbooks decided" say "Campbooks' model" or name a fact instead of
  quoting a matched sentence.
- A `pay` need is an accepted `asks_action` on mail the model sorted to
  Receipts; a person sending an invoice by hand is a `do`.
- Without a trusted `decide` role nothing is sorted out of Primary and no
  sure need is raised beyond what the facts settle. That is the honest state:
  no answer is better than a wrong one.
- Turning a hosted model on for real mail is a product decision about where
  mail goes (Jev's region, retention and processing terms), taken per
  instance, not in code.
- The eval set is still fictional. The operating points it gives are a floor
  to start from; a real-mail eval, from members' corrections, replaces it.

## Alternatives considered
- **Keep and fix the rules.** Each fix is another guess about language; the
  "?" rule was one such fix.
- **Laya alone, now.** Near chance without fine-tuning, and fine-tuning needs
  labelled real mail we do not have yet.
- **An instruct model through `complete`-style chat** (ADR 0009's adapter).
  Measured close to Jev on the same mail, 8 times slower and about twice the
  cost, with no probabilities to gate on. It stays a valid `decide` provider.
