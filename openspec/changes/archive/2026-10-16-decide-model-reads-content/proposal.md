# Proposal

Modules: `conversations` (streams, the model pass, needs, the upgrade),
`campbooks-ai` (`decide`: the facts strategy, the endpoint path, the eval),
`xtask` (eval), docs (ADR 0018, AGENTS.md).
Tracking: https://github.com/notacamp/campbooks-cloud/issues/144

## Why

In production, notifications are read as questions. "Your order has shipped.
Questions? Contact our support team." raises a Reply need. The cause is a
rule in `campbooks_ai::decide::signals::email`: any `?` in the text answers
`needs_reply` true at 0.88, the confidence the policy acts on. That file is
about 2,400 lines of phrase lists and regular expressions in four languages,
and it is the only source allowed to act: the policy lets no model answer act.
The precision it was credited with was measured on fictional mail written
with the rules.

Laya, the configured `decide` model, fails its answers check in production
(campbooks-cloud#128): untuned, it is near chance. A hosted decision model,
Jev, answered the same questions correctly on the same mail (campbooks-cloud#144,
ADR 0018): 1 of 10 notifications read as needing a reply against Laya's 10
of 10, 100% on streams, 0.4 s per message.

## What Changes

- **ADR 0018.** Understanding email content is a model's job; deterministic
  code reads structured facts only and may only say "no". Amends ADR 0005 (a
  hosted decision model may serve `decide`, as an instance setting) and ADR
  0009 (Jev joins Laya).
- **Facts replace the email signals.** `EmailSignals` becomes `EmailFacts`,
  built from a `Facts` value that holds no subject, body or attachment name.
  It answers only `needs_reply` false: list mail (`List-Id`,
  `List-Unsubscribe`, `Precedence: bulk|list|junk`), automatic mail
  (`Auto-Submitted` other than `no`), a no-reply sender address. It abstains
  on every other question. Every content regex, phrase list and the date
  resolver are removed. A test keeps the facts module free of content.
- **The model's answers act.** The policy is regenerated from an eval run
  that includes Jev: a model answer acts from the operating point where it
  holds its question's target on `test`, as the rules' did. Facts act on
  their "no". The model pass places a conversation's stream when its stream
  answer is accepted and no member or earlier message placed it.
- **Needs from the model.** `pay` is an accepted `asks_action` on a message
  whose accepted stream is `receipts`; `reply` and `do` as before. A need
  carries no due date until extraction exists (follow-up).
- **A configurable decide endpoint path.** `CAMPBOOKS_AI__DECIDE__PATH`
  (default `systemone` for the Laya protocol, `chat/completions` for OpenAI),
  so OpenRouter's `decisions` endpoint is reachable. The eval harness gains a
  `jev` strategy with `CAMPBOOKS_EVAL_JEV_*`.
- **Upgrade.** A migration marks every stored rules answer of an older envelope
  version as not acted and queues every inbox for the needs backfill, so the
  needs the old rules raised are withdrawn; the backfill asks the model again
  about each conversation's newest inbound message within the decide window
  that has no checked model answer.
- **AGENTS.md** gains the rule, with its enforcement.

## Non-goals

- Turning Jev on for any real instance. That is a configuration change in the
  private cloud repo, made after the owner signs off on where mail goes.
- Extracting due dates or the asking sentence (`complete`; follow-up issue).
- The sender running score (campbooks-cloud#145).
- A real-mail eval set, and fine-tuning Laya.
- New UI. The reader's "How Campbooks decided" panel and the need chips keep
  their shape; only the reasons they name change.
