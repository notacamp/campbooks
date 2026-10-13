# 0009 — The `decide` role is a typed decision model (Laya), not a chat model

**Status:** Accepted · 2026-10-12 · amends [0005](0005-open-weight-ai-by-role.md)

## Context
ADR 0005 put every AI role behind the OpenAI-compatible chat protocol and
described `decide` as "a small instruct model with JSON-schema output". Most
of what the product decides is discrete — which stream a message belongs to,
whether it asks something of the user, how urgent it is, which document type
a file is, whether a bank line matches an invoice. Asking a generative model
for those means parsing text, paying generation latency on every message, and
getting no usable probability to gate on.

Laya (ConvAI Innovations, Apache-2.0) is an open-weight, non-autoregressive
decision model: it takes a *state* (text, an email, JSON) and typed
*questions*, and returns calibrated-probability answers in one forward pass.
It has three question types — `choice` (one label of a set, with a
probability per label), `score` (a position on an ordered rubric) and `noul`
(probability that a statement is true). Checkpoints: English
(ModernBERT-large, 421M, 512 tokens), multilingual (mmBERT-base, 322M, 1024
tokens by default, up to 8192) and a fine-tuned typed-decisions checkpoint.
It is served by `laya-serve` on `POST /v1/systemone`.

## Decision
- **`decide` speaks a typed decision protocol**: `decide(state, questions) →
  answers`, where each answer carries the result, per-option probabilities
  and a confidence. Callers never parse free text from `decide`.
- **Laya is the default `decide` model**, multilingual checkpoint by default
  (Campbooks ships in en/pt/es/fr), served by `laya-serve`.
- **An instruct model remains a valid `decide` provider** through an adapter
  in `campbooks-ai` that answers the same typed questions with
  JSON-schema-constrained output (probabilities from logprobs when the server
  returns them, otherwise no probability). Users can point `decide` at either.
- **Extraction is not a decision.** Pulling fields out of a document, an ask
  out of an email or a due date out of a sentence is generation with a schema:
  it uses `complete` (or `vision` for images). `decide` may *gate* it ("does
  this email ask the user for something?") so `complete` runs only when needed.
- **Confidence gates every automatic action.** Below a per-question threshold,
  the product asks the user instead of acting. The user's corrections are
  stored as labelled examples, which recalibrate the per-question temperature
  (Laya ships overconfident) and form the eval set for that question.

## Consequences
- Questions are designed for the model's limits: about 20 options per
  `choice` at most (split larger sets into a coarse-to-fine pair of choices),
  ordinal `score` questions used sparingly (weakest type), and `noul`
  answers checked against a two-option `choice` where label bias shows up.
- The *state* passed to `decide` is a compact, deterministic summary built by
  the owning module (headers, sender kind, the first part of the body), not a
  raw document, so it fits the context window.
- At ~0.3–0.4B parameters, `decide` runs acceptably on CPU, so self-hosters
  without a GPU get the sorting features; only `complete` and `vision` need
  bigger hardware or a hosted endpoint.
- There is no official container image: the `ai` compose profile builds a
  small one. `laya-serve` binds every interface without authentication unless
  `LAYA_API_KEY` is set, so the service is only reachable on the compose
  network and always has a key.
- The AI gateway issue owns the protocol, the adapter, the catalog entry and
  the per-question evals.

## Alternatives considered
- Instruct model with constrained JSON for every decision (ADR 0005 as
  written) — slower per message, no reliable probabilities, parsing failures.
- Training a classifier per question — no zero-shot start, a model per
  question to ship and maintain.

## Notes
- **2026-10-13, first integration (campbooks-cloud#58).** The decision stands;
  one claim above needs a caveat. The multilingual checkpoint ships with no
  fitted temperature, and its zero-shot answers can be confidently wrong. On a
  sample invoice email (CPU, 6 cores, about 0.3–0.6 s per three-question call,
  about 2 GB of RAM), it chose the right category in English (0.99) but
  "newsletter" for the same email in Portuguese (0.9997), and gave P(true)
  ≈ 0.003 to "the sender asks the recipient to do something" in both
  languages, also when asked as a two-option `choice`. The model card reports
  the base checkpoints near chance on its typed-decisions benchmark zero-shot.
  So the probabilities are not calibrated until we refit them, and a high
  confidence is not, by itself, a reason to act. Until a question has an eval
  set from user corrections that shows it works, its answers are suggestions,
  never automatic actions. Fine-tuning a checkpoint on our own decisions is the
  expected way to make them reliable.
