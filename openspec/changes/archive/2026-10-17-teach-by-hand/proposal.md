# Proposal

## Why

A member typed into Teach Scout a sentence naming their two accounting firms
without an address (like "anything from my accountants at northtax or atelier
conseils always matters") and was told "Scout couldn't turn that into a
rule". The production log said `compile outcome=Unclear proposals=0
well_formed=false`: the model's answer never parsed, so the checks never ran,
and the page blamed the member's wording for the model's failure. The
`complete` role in production is a reasoning model, compile gave it 800
tokens for thinking and answer together, and some models wrap their JSON in a
Markdown code fence even when a JSON response format is asked for. The log
had nothing to tell these apart.

The member also wants to make rules through the page without AI at any time,
not only after the model failed or when there is none. And the builder asked
for a sender's address typed from memory, where everywhere else senders are
found by name (campbooks-cloud#205, PR #521).

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/217.

## What Changes

- **Compile survives a reasoning model** (`crates/modules/assistant`, compile
  slice): up to 4096 tokens, so the thinking leaves room for the answer.
- **A fenced answer is read** (`crates/ai`, `ChatResponse::json`): one
  Markdown code fence (` ```json ` or ` ``` `) and the whitespace around it are
  dropped before parsing. Nothing else is repaired.
- **An unreadable answer is the model's failure**: 503 `compile_failed` (the
  page offers Try again or the builder), never the `unclear` outcome. The log
  line says why the model stopped, how many tokens it wrote and how long its
  answer was, and a warning names the kind of failure (`empty`, `truncated`,
  `not_json`), never the content.
- **"Build a rule yourself", any time** (`clients/app`, assistant module): a
  button next to the privacy line opens the rule builder with no model call;
  "Tell Scout in a sentence instead" goes back. The builder's lead says why it
  is shown (no model here, the sentence failed, or the member chose it). An
  `unclear` or `unsupported` answer offers the builder too.
- **The builder finds senders by name**: its "It's from" condition uses the
  same filter box as Scout's questions; each ticked sender (an address or a
  domain) becomes its own rule card, since a rule's conditions all have to
  hold. The sender address and sender domain conditions become this one.

Modules: `assistant` (server and client) and `campbooks-ai`. No new module,
dependency, endpoint, scope, event or migration.

## Non-goals

- No change to the checks on what the model proposes: a `who` phrase still
  has to come from the sentence and is still answered by the member.
- No repair of truncated or otherwise broken JSON, and no reading of prose.
- No new conditions or actions in the builder, and no OR inside one rule.
- No change to the `complete` model chosen in production.
