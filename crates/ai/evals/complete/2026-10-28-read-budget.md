# Scout's read: how much room and how much thinking (campbooks-cloud#375)

The question: in prod, 6 `complete` answers in 7 days came back empty with
`finish_reason=length` (5 in `conversations.read_message`, 1 in the
`rule_notices` grouping). The model (`z-ai/glm-5.3-flash` on OpenRouter)
thinks inside `max_tokens`, so it can spend all 4096 and write nothing. Which
setting stops that, at what cost?

## Method

- The read request was rebuilt exactly as `read_message/prompt.rs` builds it:
  the fixed instruction (taken from the source), the strict `message_read`
  JSON schema, temperature 0.2, and an `Input` object with an open need
  (`reply` when the item is labelled `needs_reply`, else `review`), dates on,
  Europe/Lisbon, no known items or open tasks.
- 50 emails from `crates/ai/evals/decide/mail-triage/items/` (all four
  languages), the 50 with the longest bodies. The bodies are short (the
  longest is 913 characters), so real mail will think longer than this.
- Four arms, one call per email per arm, 5 in parallel, run on 2026-10-28:
  - A: today (no effort, `max_tokens` 4096)
  - B: `reasoning: {"effort": "low"}`, 4096
  - C: no effort, 8192
  - D: low effort, 8192
- "Parses" means the answer is JSON with the schema's required keys and the
  right types (a light check, not a full JSON-schema validator). Cost is
  OpenRouter's `usage.cost`.

The set is fictional and agent-written, and it is small: 50 reads per arm.
Treat the rates as a direction, not a measurement of prod.

## Results

| Arm | Effort | max_tokens | Empty (length, no content) | Parses | p50 s | p95 s | Cost per 1,000 reads (USD) | Mean / max completion tokens |
|-----|--------|-----------|----------------------------|--------|-------|-------|----------------------------|------------------------------|
| A | default | 4096 | 3 of 50 | 46 of 50 | 7.4 | 18.4 | 0.53 | 1375 / 4096 |
| B | low | 4096 | 0 | 50 of 50 | 5.0 | 11.1 | 0.25 | 687 / 2180 |
| C | default | 8192 | 0 | 48 of 50 | 9.0 | 34.4 | 0.79 | 2176 / 8192 |
| D | low | 8192 | 0 | 50 of 50 | 3.6 | 9.7 | 0.21 | 635 / 2123 |

Other failures: arm A had one more answer cut at 4096 tokens with 559
characters of unfinished JSON; arm C had two answers cut at 8192 tokens (1394
and 11274 characters), so a bigger budget alone lets the model ramble longer
and does not make the answer fit.

## What it shows

- The empty answers reproduce at today's setting (3 of 50, about 6%; prod saw
  about 3% of reads over the week).
- A bigger budget alone (C) removes the empty answers but is the slowest and
  dearest arm and still truncates 2 answers.
- Low effort (B, D) removes every failure, halves the cost and the tokens and
  is faster. The longest low-effort answer used 2180 tokens.
- D and B differ only in the cap. The cap costs nothing unless it is used, and
  it leaves room for the odd long answer, so the choice is D.

## Choice

- `read_message` and the `rule_notices` grouping: `reasoning.effort = low`,
  `max_tokens = 8192` (arm D). The grouping answer is a short list of
  phrases, so it is no harder than a read; it was not measured on its own.
- An empty answer at the limit is asked again once, in the same job, with
  twice the budget and low effort, before it counts as a failed call.
- The other `complete` calls capped at 4096 (the reply suggestions and the
  assistant's compile) show no empty answers in prod over the same week, so
  they are left as they are.

## Caveats

- Emails here are short; the quality of the summaries was not scored, only
  whether the answer exists and parses. Low effort changes how the model
  thinks, so the summaries should be spot-checked on staging.
- One run per arm, no repeats.
