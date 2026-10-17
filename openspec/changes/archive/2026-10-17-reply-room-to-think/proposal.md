## Why

Scout's reply suggestions and drafts (campbooks-cloud#168, #524) ask the
`complete` role with a token budget of 200 (suggestions) and 900 (drafts).
In production `complete` is GLM-5.3 Flash, a reasoning model whose thinking
is spent from the same `max_tokens` before it writes its answer. Teach Scout
hit exactly this at 800 tokens: the answer came back empty or cut short, and
the member saw a failure (campbooks-cloud#217, fixed in #533). Reply would
fail the same way, and its log would not say why.

## What Changes

- Both reply requests give the model 4,096 tokens, so a reasoning model has
  room to think and still answer. The answer stays bounded by the schema and
  the existing limits (three intents of 80 characters, a reply of 4,000).
- When an answer does not parse, the warning says why the model stopped
  (`finish_reason`), whether the answer was empty, cut short or not JSON, and
  its size, never its text.
- `ChatResponse::finish_kind` names the stop reason with a few known words for
  every caller; Teach Scout's compile uses it in place of its own copy.

## Impact

- `crates/ai/src/gateway.rs`, `features/reply/{mod,prompt}.rs`,
  `assistant/src/features/compile/mod.rs`.
- Cost: a call may use more output tokens when the model thinks longer; the
  per-member hourly allowance and the workspace's daily AI limit are
  unchanged.
